//! process.* 工具（设计文档 #20）：查看进程、启动应用、终止进程

use super::registry::{param_str, param_u64};
use crate::state::AppState;
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use sysinfo::{Pid, ProcessesToUpdate, System};

/// sysinfo 的 CPU 使用率需要两次刷新（间隔至少 ~200ms）才有意义。
fn refresh_all() -> System {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys
}

pub fn list(input: &Value) -> Result<Value, String> {
    let filter = param_str(input, "filter").unwrap_or("").to_lowercase();
    let limit = param_u64(input, "limit").unwrap_or(100).clamp(1, 1000) as usize;

    let sys = refresh_all();
    let mut procs: Vec<Value> = sys
        .processes()
        .iter()
        .map(|(pid, p)| {
            json!({
                "pid": pid.as_u32(),
                "name": p.name().to_string_lossy(),
                "cpu": p.cpu_usage(),
                "memoryMB": p.memory() / 1024 / 1024,
                "status": format!("{:?}", p.status()),
            })
        })
        .filter(|v| {
            filter.is_empty()
                || v["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&filter)
        })
        .collect();
    // 按内存占用降序，取前 N 个
    procs.sort_by(|a, b| {
        b["memoryMB"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["memoryMB"].as_u64().unwrap_or(0))
    });
    procs.truncate(limit);

    Ok(json!({ "processes": procs, "count": procs.len() }))
}

pub fn inspect(input: &Value) -> Result<Value, String> {
    let pid_raw = param_u64(input, "pid").ok_or("缺少参数 pid")?;
    let pid = Pid::from_u32(pid_raw as u32);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let p = sys
        .process(pid)
        .ok_or_else(|| format!("进程 {pid_raw} 不存在"))?;
    let cmd = p
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(json!({
        "pid": pid_raw,
        "name": p.name().to_string_lossy(),
        "exe": p.exe().map(|e| e.display().to_string()).unwrap_or_default(),
        "cmd": cmd,
        "cwd": p.cwd().map(|c| c.display().to_string()).unwrap_or_default(),
        "parentPid": p.parent().map(|pp| pp.as_u32()),
        "cpu": p.cpu_usage(),
        "memoryMB": p.memory() / 1024 / 1024,
        "startTime": p.start_time(),
        "status": format!("{:?}", p.status()),
    }))
}

/// 启动应用（分离模式：不等待退出，进程独立运行）
pub fn start(state: &AppState, task_id: Option<&str>, input: &Value) -> Result<Value, String> {
    let command = param_str(input, "command")
        .ok_or("缺少参数 command")?
        .to_string();
    if command.trim().is_empty() {
        return Err("启动命令不能为空".into());
    }
    if let Some(dir) = param_str(input, "cwd") {
        crate::security::sandbox::check_path(dir, crate::security::sandbox::PathOp::Read)?;
    }

    let mut cmd = Command::new(&command);
    if let Some(args) = input.get("args").and_then(|v| v.as_array()) {
        for a in args.iter().filter_map(|v| v.as_str()) {
            cmd.arg(a);
        }
    }
    if let Some(dir) = param_str(input, "cwd") {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| format!("启动应用失败: {e}（请检查应用名或路径）"))?;
    let pid = child.id();

    if let Some(tid) = task_id {
        state
            .running_pids
            .lock()
            .unwrap()
            .entry(tid.to_string())
            .or_default()
            .push(pid);
    }

    Ok(json!({ "pid": pid, "name": command }))
}

pub fn kill(input: &Value) -> Result<Value, String> {
    let pid_raw = param_u64(input, "pid").ok_or("缺少参数 pid")?;
    let pid = Pid::from_u32(pid_raw as u32);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let p = sys
        .process(pid)
        .ok_or_else(|| format!("进程 {pid_raw} 不存在"))?;
    let name = p.name().to_string_lossy().to_string();
    let ok = p.kill();
    if ok {
        Ok(json!({ "pid": pid_raw, "name": name, "killed": true }))
    } else {
        Err(format!("终止进程 {pid_raw}（{name}）失败（可能权限不足）"))
    }
}
