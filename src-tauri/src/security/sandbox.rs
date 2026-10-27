//! 路径沙箱：限制对系统目录的破坏性访问（设计文档 #7 security/sandbox.rs）

use std::path::{Component, Path, PathBuf};

/// 系统保护目录（写入 / 删除 / 移动目标不允许位于这里）
const PROTECTED_PREFIXES: &[&str] = &[
    "c:\\windows",
    "c:\\program files",
    "c:\\program files (x86)",
    "c:\\programdata\\microsoft",
    // 类 Unix（开发/测试环境）
    "/bin",
    "/boot",
    "/etc",
    "/lib",
    "/lib64",
    "/sbin",
    "/usr",
    "/var",
    "/sys",
    "/proc",
];

/// 规范化路径并解析 `..`（不访问文件系统）
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// 去掉 Windows `\\?\` 前缀，便于与保护目录前缀比较
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

/// 解析真实路径：对不存在的目标，逐级向上找到已存在祖先再拼接剩余部分。
/// 这样即使目标文件尚未创建，也能识别其父目录中的符号链接。
fn resolve_real(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&existing) {
            let mut out = strip_verbatim(real);
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        let Some(parent) = existing.parent() else {
            return path.to_path_buf();
        };
        if let Some(name) = existing.file_name() {
            tail.push(name.to_os_string());
        }
        existing = parent.to_path_buf();
    }
}

/// 规范化路径：解析 `..`、补全相对路径并尽可能解析符号链接
pub fn canonicalize(path: &str) -> PathBuf {
    let p = Path::new(path);
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    };
    resolve_real(&normalize(&absolute))
}

fn is_protected(path: &Path) -> bool {
    // 统一分隔符后再比较：Windows 上 `/etc/passwd` 会被规范化为 `\etc\passwd`，
    // 如果不归一化就会绕过 Unix 保护目录的判定。
    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    PROTECTED_PREFIXES.iter().any(|prefix| {
        let prefix = prefix.replace('\\', "/");
        lower == prefix || lower.starts_with(&format!("{prefix}/"))
    })
}

/// 操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOp {
    /// 读取类（list/read/stat/search）
    Read,
    /// 写入类（write/mkdir/copy/move/rename/delete）
    Mutate,
}

/// 检查路径是否允许访问。返回 Err(原因) 表示拒绝。
pub fn check_path(path: &str, op: PathOp) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("路径不能为空".into());
    }
    let raw = normalize(Path::new(path));
    let resolved = canonicalize(path);
    // 同时检查输入形态与真实路径：既覆盖符号链接/相对路径，
    // 也保留 Windows 上 `/etc`、`/usr` 这类 Unix 风格路径的保护语义。
    if op == PathOp::Mutate && (is_protected(&raw) || is_protected(&resolved)) {
        return Err(format!("拒绝访问系统保护目录: {}", resolved.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_parent_dirs() {
        let p = canonicalize("D:/data/project/../config.json");
        // 用 PathBuf 比较，避免 Windows 反斜杠与测试字符串不一致
        assert_eq!(p, PathBuf::from("D:/data/config.json"));
        assert!(!p.to_string_lossy().contains(".."));
    }

    #[test]
    fn blocks_mutation_in_system_dirs() {
        assert!(check_path("C:\\Windows\\System32\\evil.dll", PathOp::Mutate).is_err());
        assert!(check_path("C:\\Program Files\\app\\x.txt", PathOp::Mutate).is_err());
        assert!(check_path("/etc/passwd", PathOp::Mutate).is_err());
    }

    #[test]
    fn allows_reads_in_system_dirs() {
        assert!(check_path("C:\\Windows\\System32\\drivers\\etc\\hosts", PathOp::Read).is_ok());
    }

    #[test]
    fn allows_user_paths() {
        assert!(check_path("D:/Downloads/a.pdf", PathOp::Mutate).is_ok());
        assert!(check_path("C:\\Users\\admin\\Downloads\\a.pdf", PathOp::Mutate).is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(check_path("  ", PathOp::Read).is_err());
    }

    #[test]
    fn blocks_mutation_through_symlink() {
        let dir = std::env::temp_dir().join(format!(
            "zdesk-link-{}-{}",
            std::process::id(),
            crate::database::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let link = dir.join("link");

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", &link).unwrap();
            assert!(
                check_path(&link.join("passwd").to_string_lossy(), PathOp::Mutate).is_err(),
                "不可通过符号链接写入 /etc"
            );
            let _ = std::fs::remove_file(&link);
        }

        #[cfg(windows)]
        {
            // 普通用户可能没有创建符号链接的权限，创建失败则跳过
            if std::os::windows::fs::symlink_dir("C:\\Windows", &link).is_ok() {
                assert!(
                    check_path(&link.join("evil.dll").to_string_lossy(), PathOp::Mutate).is_err(),
                    "不可通过符号链接写入 C:\\Windows"
                );
                let _ = std::fs::remove_dir(&link);
            }
        }

        let _ = std::fs::remove_dir(&dir);
    }
}
