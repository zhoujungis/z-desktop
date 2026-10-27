//! 风险分级（设计文档 #19 / #30）

use serde::{Deserialize, Serialize};

/// 权限等级（设计文档 #30：SAFE / LOW / MEDIUM / HIGH / CRITICAL）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "safe",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }
}

/// 危险系统操作（CRITICAL，默认禁止，设计文档 #19 / #30）
const CRITICAL_PATTERNS: &[&str] = &[
    // Windows
    "format ",
    "format.exe",
    "shutdown",
    "restart-computer",
    "stop-computer",
    "bcdedit",
    "vssadmin delete",
    "cipher /w",
    "reg delete hklm",
    "reg add hklm\\system\\currentcontrolset\\control\\securityproviders",
    "set-executionpolicy unrestricted",
    "remove-item c:\\windows",
    "rd /s c:\\windows",
    "del /f /s /q c:\\windows",
    "rmdir /s c:\\windows",
    "diskpart",
    "icacls c:",
    "net user administrator",
    "net stop windefend",
    "sc config windefend",
    "set-mppreference -disablerealtimemonitoring",
    "wmic shadowcopy delete",
    // 跨平台
    "mkfs",
    "dd if=",
    "rm -rf /",
    "rm -rf ~",
    "rm -rf /*",
    ":(){:|:&};:",
    "> /dev/sda",
    "chmod -r 000 /",
    "chown -r root /",
];

/// 高风险命令特征（HIGH，每次确认，设计文档 #19）
const HIGH_PATTERNS: &[&str] = &[
    "remove-item",
    "rd /s",
    "rmdir /s",
    "del /f",
    "del /q /s",
    "erase ",
    "reg delete",
    "reg add",
    "reg import",
    "taskkill /f",
    "stop-process",
    "kill -9",
    "pkill",
    "npm publish",
    "pip install",
    "winget install",
    "choco install",
    "apt install",
    "apt remove",
    "curl | sh",
    "curl | bash",
    "iex (iwr",
    "invoke-expression",
    "schtasks /create",
    "net user",
    "net localgroup",
    "attrib -s -h",
    // 会改写历史 / 工作区的 git 命令（git status 等只读命令不受影响）
    "git branch -d",
    "git branch --delete",
    "git reset --hard",
    "git clean",
    "git push",
    "git restore",
    "git checkout --",
];

/// 安全命令特征（SAFE，可配置自动执行，设计文档 #19）
const SAFE_PATTERNS: &[&str] = &[
    "dir",
    "ls",
    "get-childitem",
    "get-content",
    "cat",
    "type ",
    "head",
    "tail",
    "echo",
    "pwd",
    "cd",
    "where",
    "which",
    "whoami",
    "hostname",
    "date",
    "node --version",
    "node -v",
    "npm --version",
    "npm -v",
    "npm ls",
    "npm list",
    "npm view",
    "npm run lint",
    "python --version",
    "python -v",
    "pip --version",
    "pip list",
    "git status",
    "git log",
    "git diff",
    "git branch",
    "git show",
    "git remote -v",
    "cargo --version",
    "cargo check",
    "cargo test --no-run",
    "rustc --version",
    "systeminfo",
    "tasklist",
    "get-process",
    "ipconfig",
    "ping",
    "netstat",
    "tree",
    "findstr",
    "select-string",
    "wc",
    "stat",
    "du -",
    "df -",
];

fn normalize(cmd: &str) -> String {
    cmd.trim().to_lowercase()
}

fn matches_any(cmd: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| cmd.contains(p))
}

/// 把命令拆成独立片段：`;`、`&&`、`||`、`|`、换行都视为命令边界。
///
/// 这是必要的：只看整条命令是否包含安全关键字，会把
/// `echo ok && rm -rf ./data` 误判为 SAFE 并自动执行。
fn split_segments(cmd: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ';' | '\n' | '\r' => {
                let seg = current.trim();
                if !seg.is_empty() {
                    segments.push(seg.to_string());
                }
                current.clear();
            }
            '&' | '|' => {
                let seg = current.trim();
                if !seg.is_empty() {
                    segments.push(seg.to_string());
                }
                current.clear();
                // 吞掉 && 与 || 的第二个字符
                if matches!(chars.peek(), Some('&') | Some('|')) {
                    chars.next();
                }
            }
            _ => current.push(c),
        }
    }
    let seg = current.trim();
    if !seg.is_empty() {
        segments.push(seg.to_string());
    }
    segments
}

/// 首 token 必须构成完整单词，避免 `cdx.exe` 命中 `cd`、`catalog` 命中 `cat`。
fn matches_safe_prefix(cmd: &str) -> bool {
    SAFE_PATTERNS.iter().any(|pattern| {
        let Some(rest) = cmd.strip_prefix(pattern) else {
            return false;
        };
        if rest.is_empty() {
            return true;
        }
        let last = pattern.chars().last().unwrap_or(' ');
        // 以空白 / 符号结尾的模式本身就是前缀（如 "type "、"du -"）
        if !last.is_alphanumeric() {
            return true;
        }
        rest.starts_with(|c: char| c.is_whitespace() || c == '-')
    })
}

fn is_read_only(segment: &str) -> bool {
    // 重定向意味着写入，不按只读命令处理
    if segment.contains('>') {
        return false;
    }
    matches_safe_prefix(segment)
}

/// 针对常见 shell 原语的破坏性判断（如 rm / del / Remove-Item / mv 覆盖等）
fn looks_destructive(segment: &str) -> bool {
    const DESTRUCTIVE_TOKENS: &[&str] = &[
        "rm", "rmdir", "rd", "del", "erase", "unlink", "shred", "mv", "move", "truncate",
    ];
    let first = segment.split_whitespace().next().unwrap_or("");
    if DESTRUCTIVE_TOKENS.contains(&first) {
        return true;
    }
    // 管道重定向覆盖系统文件等
    segment.contains(">/dev/sd") || segment.contains(">c:\\") || segment.contains(">>c:\\windows")
}

fn classify_segment(segment: &str) -> RiskLevel {
    if matches_any(segment, CRITICAL_PATTERNS) {
        return RiskLevel::Critical;
    }
    if matches_any(segment, HIGH_PATTERNS) {
        return RiskLevel::High;
    }
    if looks_destructive(segment) {
        return RiskLevel::High;
    }
    if is_read_only(segment) {
        return RiskLevel::Safe;
    }
    RiskLevel::Medium
}

/// 判定 Shell 命令风险等级（设计文档 #19 Shell 安全）
///
/// 整条命令按连接符拆段后逐段判定，取最高风险；只有所有片段都是
/// 只读命令时才判定为 SAFE，防止 `安全命令 && 危险命令` 的绕过。
pub fn classify_command(command: &str) -> RiskLevel {
    let cmd = normalize(command);
    let segments = split_segments(&cmd);
    if segments.is_empty() {
        return RiskLevel::Medium;
    }
    segments
        .iter()
        .map(|s| classify_segment(s))
        .max()
        .unwrap_or(RiskLevel::Medium)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_commands() {
        assert_eq!(classify_command("git status"), RiskLevel::Safe);
        assert_eq!(classify_command("npm --version"), RiskLevel::Safe);
        assert_eq!(classify_command("node -v"), RiskLevel::Safe);
        assert_eq!(
            classify_command("Get-ChildItem D:\\Downloads"),
            RiskLevel::Safe
        );
        assert_eq!(classify_command("dir C:\\Users"), RiskLevel::Safe);
        assert_eq!(classify_command("cargo check"), RiskLevel::Safe);
        assert_eq!(classify_command("tasklist"), RiskLevel::Safe);
    }

    #[test]
    fn medium_commands() {
        assert_eq!(classify_command("npm test"), RiskLevel::Medium);
        assert_eq!(classify_command("npm install"), RiskLevel::Medium);
        assert_eq!(classify_command("python main.py"), RiskLevel::Medium);
        assert_eq!(classify_command("node build.js"), RiskLevel::Medium);
    }

    #[test]
    fn high_commands() {
        assert_eq!(
            classify_command("Remove-Item D:\\temp\\a.txt"),
            RiskLevel::High
        );
        assert_eq!(classify_command("del /f /q a.txt"), RiskLevel::High);
        assert_eq!(
            classify_command("taskkill /f /im notepad.exe"),
            RiskLevel::High
        );
        assert_eq!(
            classify_command("reg delete HKLM\\Software\\X"),
            RiskLevel::Critical
        );
        assert_eq!(classify_command("rm -rf build"), RiskLevel::High);
    }

    #[test]
    fn critical_commands() {
        assert_eq!(classify_command("format C:"), RiskLevel::Critical);
        assert_eq!(classify_command("shutdown /s /t 0"), RiskLevel::Critical);
        assert_eq!(
            classify_command("Set-MpPreference -DisableRealtimeMonitoring $true"),
            RiskLevel::Critical
        );
        assert_eq!(classify_command("mkfs.ext4 /dev/sda1"), RiskLevel::Critical);
    }

    #[test]
    fn levels_order() {
        assert!(RiskLevel::Safe < RiskLevel::Low);
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn chained_destructive_commands_are_not_safe() {
        // 安全命令 + 危险命令：整体必须按最高风险处理
        assert_eq!(
            classify_command("echo ok && rm -rf ./build"),
            RiskLevel::High
        );
        assert_eq!(
            classify_command("cd D:\\ && del important.txt"),
            RiskLevel::High
        );
        assert_eq!(
            classify_command("git status; git clean -fdx"),
            RiskLevel::High
        );
        assert_eq!(
            classify_command("dir | Remove-Item -Recurse"),
            RiskLevel::High
        );
        assert_eq!(
            classify_command("echo hi & shutdown /s"),
            RiskLevel::Critical
        );
        // 管道只读命令仍是 SAFE
        assert_eq!(classify_command("dir | findstr foo"), RiskLevel::Safe);
        assert_eq!(classify_command("git status && git log"), RiskLevel::Safe);
    }

    #[test]
    fn safe_prefix_requires_word_boundary() {
        // 不能因为包含 "cd" / "cat" / "ls" 就误判为只读
        assert_eq!(classify_command("cdx.exe /payload"), RiskLevel::Medium);
        assert_eq!(classify_command("catalog.exe"), RiskLevel::Medium);
        assert_eq!(classify_command("lsblk"), RiskLevel::Medium);
        assert_eq!(classify_command("cd D:\\work"), RiskLevel::Safe);
    }

    #[test]
    fn redirection_is_not_read_only() {
        assert_eq!(classify_command("echo hi > out.txt"), RiskLevel::Medium);
        assert_eq!(classify_command("echo hi"), RiskLevel::Safe);
    }

    #[test]
    fn destructive_git_commands_need_approval() {
        assert_eq!(classify_command("git branch -D main"), RiskLevel::High);
        assert_eq!(classify_command("git push origin main"), RiskLevel::High);
        assert_eq!(classify_command("git reset --hard HEAD~1"), RiskLevel::High);
        assert_eq!(classify_command("git branch"), RiskLevel::Safe);
        assert_eq!(classify_command("git log --oneline"), RiskLevel::Safe);
    }
}
