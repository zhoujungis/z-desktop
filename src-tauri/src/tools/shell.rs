//! shell.* 工具（设计文档 #18 / #19）
//!
//! 支持 PowerShell / CMD / WSL / Git Bash / Unix Sh；带超时与用户取消（设计文档 #40），
//! 结束时终止整个进程树，并把子进程 PID 登记到 AppState 以便 Stop 时回收。

use super::registry::{param_str, param_u64};
use crate::state::AppState;
use serde_json::{json, Value};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_OUTPUT: usize = 100_000;
const OUTPUT_WAIT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy)]
enum ShellKind {
    PowerShell,
    Cmd,
    Wsl,
    GitBash,
    UnixSh,
}

fn resolve_shell(input: &Value) -> Result<ShellKind, String> {
    let name = param_str(input, "shell").unwrap_or("");
    Ok(match name {
        "powershell" => ShellKind::PowerShell,
        "cmd" => ShellKind::Cmd,
        "wsl" => ShellKind::Wsl,
        "gitbash" => ShellKind::GitBash,
        "sh" | "bash" => ShellKind::UnixSh,
        "" => {
            if cfg!(windows) {
                ShellKind::PowerShell
            } else {
                ShellKind::UnixSh
            }
        }
        other => return Err(format!("不支持的 shell: {other}")),
    })
}

fn build_command(kind: ShellKind, command: &str, cwd: Option<&str>) -> Result<Command, String> {
    let mut cmd = match kind {
        ShellKind::PowerShell => {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-Command", command]);
            c
        }
        ShellKind::Cmd => {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command);
            c
        }
        ShellKind::Wsl => {
            let mut c = Command::new("wsl");
            c.args(["bash", "-c", command]);
            c
        }
        ShellKind::GitBash | ShellKind::UnixSh => {
            let mut c = Command::new("bash");
            c.args(["-c", command]);
            c
        }
    };
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(cmd)
}

/// 终止整个进程树（Windows: taskkill /T /F；Unix: kill -9 仅直接子进程）
fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).spawn();
    }
}

fn truncate_tail(s: &str) -> String {
    if s.len() <= MAX_OUTPUT {
        return s.to_string();
    }
    let mut start = s.len() - MAX_OUTPUT;
    while start > 0 && !s.is_char_boundary(start) {
        start -= 1;
    }
    format!("…（已截断）{}", &s[start..])
}

/// 只保留尾部 MAX_OUTPUT 字节，避免大输出无限占用内存。
/// 返回本次写入是否发生了丢弃。
fn push_tail(buf: &mut Vec<u8>, data: &[u8]) -> bool {
    if data.len() >= MAX_OUTPUT {
        buf.clear();
        buf.extend_from_slice(&data[data.len() - MAX_OUTPUT..]);
        return true;
    }
    let overflow = (buf.len() + data.len()).saturating_sub(MAX_OUTPUT);
    if overflow > 0 {
        buf.drain(..overflow);
    }
    buf.extend_from_slice(data);
    overflow > 0
}

/// 在独立线程读取输出管道（进程被杀后管道关闭即返回）
fn drain<R: Read + Send + 'static>(pipe: Option<R>, tx: std::sync::mpsc::Sender<(String, bool)>) {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut truncated = false;
        if let Some(mut p) = pipe {
            let mut chunk = [0u8; 8192];
            loop {
                match p.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => truncated |= push_tail(&mut buf, &chunk[..n]),
                    Err(_) => break,
                }
            }
        }
        let _ = tx.send((String::from_utf8_lossy(&buf).into_owned(), truncated));
    });
}

/// 等待子进程结束（阻塞线程中执行）：轮询退出状态 / 超时 / 取消
fn wait_loop(
    mut child: std::process::Child,
    timeout_ms: u64,
    cancel: Arc<AtomicBool>,
) -> Result<Value, String> {
    let (out_tx, out_rx) = std::sync::mpsc::channel::<(String, bool)>();
    let (err_tx, err_rx) = std::sync::mpsc::channel::<(String, bool)>();
    drain(child.stdout.take(), out_tx);
    drain(child.stderr.take(), err_tx);

    let pid = child.id();
    let start = Instant::now();
    let deadline = Duration::from_millis(timeout_ms.min(3_600_000));
    let mut timed_out = false;
    let mut cancelled = false;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => return Err(format!("等待进程失败: {e}")),
        }
        if start.elapsed() >= deadline {
            timed_out = true;
            break;
        }
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    if timed_out || cancelled {
        kill_tree(pid);
        // taskkill 可能因权限等原因失败；Child::kill 作为兜底，
        // 确保 wait() 不会因为进程仍存活而永久阻塞。
        let _ = child.kill();
    }
    let status = child.wait().map_err(|e| e.to_string())?;

    let (stdout, stdout_truncated) = out_rx
        .recv_timeout(OUTPUT_WAIT)
        .unwrap_or_else(|_| (String::from("（输出读取超时）"), false));
    let (stderr, stderr_truncated) = err_rx.recv_timeout(OUTPUT_WAIT).unwrap_or_default();

    let stdout = truncate_tail(&stdout);
    let stderr = truncate_tail(&stderr);
    let stdout = if stdout_truncated {
        format!("…（已截断）{stdout}")
    } else {
        stdout
    };
    let stderr = if stderr_truncated {
        format!("…（已截断）{stderr}")
    } else {
        stderr
    };

    Ok(json!({
        "exitCode": status.code().unwrap_or(-1),
        "stdout": stdout,
        "stderr": stderr,
        "duration": start.elapsed().as_millis() as u64,
        "timedOut": timed_out,
        "cancelled": cancelled,
        "pid": pid,
    }))
}

/// 取消轮询：用户点击 Stop 后置位取消标志（设计文档 #40）
async fn poll_cancel(state: &AppState, task_id: Option<&str>, cancel: Arc<AtomicBool>) {
    let Some(tid) = task_id else {
        std::future::pending::<()>().await;
        return;
    };
    loop {
        if state.is_cancelled(tid) {
            cancel.store(true, Ordering::Relaxed);
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

pub async fn exec(state: &AppState, task_id: Option<&str>, input: &Value) -> Result<Value, String> {
    let command = param_str(input, "command")
        .ok_or("缺少参数 command")?
        .to_string();
    if command.trim().is_empty() {
        return Err("命令不能为空".into());
    }
    if let Some(dir) = param_str(input, "cwd") {
        crate::security::sandbox::check_path(dir, crate::security::sandbox::PathOp::Read)?;
    }
    let timeout_ms = param_u64(input, "timeout").unwrap_or(120_000);
    let cwd = param_str(input, "cwd").map(String::from);
    let kind = resolve_shell(input)?;
    let mut cmd = build_command(kind, &command, cwd.as_deref())?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("启动命令失败: {e}（请检查命令或 shell 是否可用）"))?;
    let pid = child.id();

    // 登记 PID：任务 Stop 时可终止（设计文档 #40）
    if let Some(tid) = task_id {
        state
            .running_pids
            .lock()
            .unwrap()
            .entry(tid.to_string())
            .or_default()
            .push(pid);
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();
    let mut handle =
        tokio::task::spawn_blocking(move || wait_loop(child, timeout_ms, cancel_for_task));

    let joined = tokio::select! {
        r = &mut handle => r,
        _ = poll_cancel(state, task_id, cancel.clone()) => handle.await,
    };

    // 注销 PID
    if let Some(tid) = task_id {
        if let Some(v) = state.running_pids.lock().unwrap().get_mut(tid) {
            v.retain(|p| *p != pid);
        }
    }

    joined.map_err(|e| format!("shell 任务异常: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_tail_keeps_utf8_boundary() {
        let s = "你好".repeat(60_000); // 240KB
        let out = truncate_tail(&s);
        assert!(out.len() <= MAX_OUTPUT + 32);
        assert!(out.starts_with("…（已截断）"));
    }

    #[test]
    fn test_truncate_tail_no_op() {
        assert_eq!(truncate_tail("hello"), "hello");
    }

    #[test]
    fn tail_buffer_keeps_only_recent_bytes() {
        let mut buf = Vec::new();
        assert!(!push_tail(&mut buf, b"START"));
        assert!(push_tail(&mut buf, &vec![b'x'; MAX_OUTPUT + 100]));
        assert_eq!(buf.len(), MAX_OUTPUT, "缓冲区不得超过上限");
        assert!(!buf.starts_with(b"START"), "最早的内容应被丢弃");
        assert!(buf.ends_with(b"x"));
    }

    #[test]
    fn tail_buffer_handles_incremental_chunks() {
        let mut buf = Vec::new();
        let mut truncated = false;
        for _ in 0..((MAX_OUTPUT / 8192) + 3) {
            truncated |= push_tail(&mut buf, &[b'a'; 8192]);
        }
        assert!(truncated, "超过上限后应标记截断");
        assert!(buf.len() <= MAX_OUTPUT);
    }
}
