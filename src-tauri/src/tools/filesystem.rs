//! filesystem.* 工具（设计文档 #17）

use super::registry::{param_bool, param_str, param_u64};
use crate::security::sandbox::{self, PathOp};
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

fn get_path(input: &Value, key: &str) -> Result<String, String> {
    param_str(input, key)
        .map(String::from)
        .ok_or_else(|| format!("缺少参数 {key}"))
}

fn mtime_to_ms(m: &fs::Metadata) -> Option<i64> {
    m.modified().ok().and_then(|t| {
        t.duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_millis() as i64)
    })
}

fn entry_json(path: &Path) -> Value {
    let meta = fs::symlink_metadata(path).ok();
    json!({
        "name": path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "path": path.to_string_lossy(),
        "isDir": meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
        "size": meta.as_ref().map(|m| if m.is_dir() { 0 } else { m.len() }).unwrap_or(0),
        "modified": meta.as_ref().and_then(mtime_to_ms),
    })
}

pub fn list(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    sandbox::check_path(&path, PathOp::Read)?;
    let depth = param_u64(input, "depth").unwrap_or(1).clamp(1, 8);
    let root = PathBuf::from(&path);

    const MAX_ENTRIES: usize = 2000;
    let mut entries: Vec<Value> = Vec::new();
    let mut truncated = false;
    if depth <= 1 {
        for entry in fs::read_dir(&root).map_err(|e| format!("读取目录失败 {path}: {e}"))? {
            if entries.len() >= MAX_ENTRIES {
                truncated = true;
                break;
            }
            let entry = entry.map_err(|e| e.to_string())?;
            entries.push(entry_json(&entry.path()));
        }
    } else {
        for entry in walkdir::WalkDir::new(&root)
            .max_depth(depth as usize)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.depth() == 0 {
                continue;
            }
            if entries.len() >= MAX_ENTRIES {
                truncated = true;
                break;
            }
            entries.push(entry_json(entry.path()));
        }
    }
    Ok(json!({
        "path": path,
        "entries": entries,
        "count": entries.len(),
        "truncated": truncated,
    }))
}

pub fn read(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    sandbox::check_path(&path, PathOp::Read)?;
    // 上限 50MB，避免 LLM 传入超大值把内存打满
    let max_bytes = param_u64(input, "max_bytes")
        .unwrap_or(2_000_000)
        .clamp(1, 50 * 1024 * 1024) as usize;
    let mut file = fs::File::open(&path).map_err(|e| format!("打开文件失败 {path}: {e}"))?;
    let total_len = file.metadata().map(|m| m.len()).unwrap_or(0);

    let mut head = vec![0u8; 8192.min(max_bytes)];
    let head_len = file.read(&mut head).map_err(|e| e.to_string())?;
    head.truncate(head_len);
    let is_binary = head.contains(&0u8);

    if is_binary {
        // 二进制：读取前 max_bytes 并返回 base64
        let mut buf = head;
        let mut rest = file.take((max_bytes as u64).saturating_sub(head_len as u64));
        rest.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        return Ok(json!({
            "path": path,
            "binary": true,
            "size": total_len,
            "truncated": total_len as usize > buf.len(),
            "content": b64,
        }));
    }

    // 文本：继续读取余下部分（上限 max_bytes）
    let mut buf = head;
    let mut rest = file.take((max_bytes as u64).saturating_sub(buf.len() as u64));
    rest.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let truncated = total_len as usize > buf.len();
    Ok(json!({
        "path": path,
        "binary": false,
        "size": total_len,
        "truncated": truncated,
        "content": String::from_utf8_lossy(&buf),
    }))
}

pub fn write(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    sandbox::check_path(&path, PathOp::Mutate)?;
    // 不能用默认空串：LLM 漏传 content 时会静默清空目标文件
    let content = param_str(input, "content").ok_or("缺少参数 content")?;
    let p = PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建父目录失败: {e}"))?;
    }
    fs::write(&p, content).map_err(|e| format!("写入文件失败 {path}: {e}"))?;
    Ok(json!({ "path": path, "bytes": content.len() }))
}

/// 通配符（* / ?）转正则
fn glob_to_regex(pattern: &str) -> String {
    // (?i)：大小写不敏感（*.pdf 应匹配 A.PDF）
    let mut out = String::from("(?i)^");
    for ch in pattern.chars() {
        match ch {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            c => out.push_str(&regex::escape(&c.to_string())),
        }
    }
    out.push('$');
    out
}

pub fn search(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    let pattern = param_str(input, "pattern").ok_or("缺少参数 pattern")?;
    sandbox::check_path(&path, PathOp::Read)?;
    let max_results = param_u64(input, "max_results")
        .unwrap_or(200)
        .clamp(1, 2000) as usize;
    let re = regex::Regex::new(&glob_to_regex(pattern)).map_err(|e| e.to_string())?;

    let mut matches: Vec<Value> = Vec::new();
    for entry in walkdir::WalkDir::new(&path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy().to_lowercase();
        if re.is_match(&file_name) {
            matches.push(entry_json(entry.path()));
            if matches.len() >= max_results {
                break;
            }
        }
    }
    Ok(json!({ "path": path, "pattern": pattern, "matches": matches, "count": matches.len() }))
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<u64, String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    let mut count = 0u64;
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let rel = entry.path().strip_prefix(from).map_err(|e| e.to_string())?;
        let target = to.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(entry.path(), &target)
                .map_err(|e| format!("复制 {} 失败: {e}", entry.path().display()))?;
            count += 1;
        }
    }
    Ok(count)
}

pub fn copy(input: &Value) -> Result<Value, String> {
    let from = get_path(input, "from")?;
    let to = get_path(input, "to")?;
    let overwrite = param_bool(input, "overwrite").unwrap_or(false);
    sandbox::check_path(&from, PathOp::Read)?;
    sandbox::check_path(&to, PathOp::Mutate)?;
    let src = PathBuf::from(&from);
    let dst = PathBuf::from(&to);
    if !src.exists() {
        return Err(format!("源不存在: {from}"));
    }
    if dst.exists() && !overwrite {
        return Err(format!("目标已存在: {to}（可设置 overwrite=true）"));
    }
    if src.is_dir() {
        // 防止把目录复制进它自己，导致 WalkDir 递归复制新文件
        let src_canon = sandbox::canonicalize(&from);
        let dst_canon = sandbox::canonicalize(&to);
        if dst_canon.starts_with(&src_canon) {
            return Err("目标目录不能位于源目录内部".into());
        }
    }
    let copied = if src.is_dir() {
        copy_dir_recursive(&src, &dst)?
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(&src, &dst).map_err(|e| format!("复制失败: {e}"))?;
        1
    };
    Ok(json!({ "from": from, "to": to, "copied": copied }))
}

pub fn move_path(input: &Value) -> Result<Value, String> {
    let from = get_path(input, "from")?;
    let to = get_path(input, "to")?;
    let overwrite = param_bool(input, "overwrite").unwrap_or(false);
    sandbox::check_path(&from, PathOp::Mutate)?;
    sandbox::check_path(&to, PathOp::Mutate)?;
    let src = PathBuf::from(&from);
    let dst = PathBuf::from(&to);
    if !src.exists() {
        return Err(format!("源不存在: {from}"));
    }
    if dst.exists() && !overwrite {
        return Err(format!("目标已存在: {to}（可设置 overwrite=true）"));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // 同卷 rename，跨卷 copy+delete
    match fs::rename(&src, &dst) {
        Ok(()) => Ok(json!({ "from": from, "to": to, "method": "rename" })),
        Err(_) => {
            let copied = if src.is_dir() {
                copy_dir_recursive(&src, &dst)?
            } else {
                fs::copy(&src, &dst).map_err(|e| format!("跨卷移动失败: {e}"))?;
                1
            };
            if src.is_dir() {
                fs::remove_dir_all(&src).map_err(|e| format!("清理源目录失败: {e}"))?;
            } else {
                fs::remove_file(&src).map_err(|e| format!("清理源文件失败: {e}"))?;
            }
            Ok(json!({ "from": from, "to": to, "method": "copy+delete", "copied": copied }))
        }
    }
}

pub fn rename(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    let new_name = param_str(input, "new_name").ok_or("缺少参数 new_name")?;
    if new_name.contains('\\') || new_name.contains('/') {
        return Err("new_name 只能是文件名，不能包含路径分隔符".into());
    }
    if new_name.trim().is_empty() || new_name == "." || new_name == ".." {
        return Err("new_name 不能为空、. 或 ..".into());
    }
    sandbox::check_path(&path, PathOp::Mutate)?;
    let src = PathBuf::from(&path);
    if !src.exists() {
        return Err(format!("路径不存在: {path}"));
    }
    let parent = src.parent().ok_or("无法获取父目录")?;
    let dst = parent.join(new_name);
    sandbox::check_path(&dst.to_string_lossy(), PathOp::Mutate)?;
    fs::rename(&src, &dst).map_err(|e| format!("重命名失败: {e}"))?;
    Ok(json!({ "from": path, "to": dst.to_string_lossy() }))
}

pub fn mkdir(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    sandbox::check_path(&path, PathOp::Mutate)?;
    fs::create_dir_all(&path).map_err(|e| format!("创建目录失败 {path}: {e}"))?;
    Ok(json!({ "path": path, "created": true }))
}

pub fn delete(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    let recursive = param_bool(input, "recursive").unwrap_or(false);
    sandbox::check_path(&path, PathOp::Mutate)?;
    let p = PathBuf::from(&path);
    let meta = fs::symlink_metadata(&p).map_err(|e| format!("路径不存在: {path}（{e}）"))?;
    if meta.file_type().is_symlink() {
        // 只删除链接本身，绝不跟随链接删除目标内容。
        // Windows 的目录符号链接要用 remove_dir，Unix 一律 remove_file。
        #[cfg(windows)]
        {
            if p.is_dir() {
                fs::remove_dir(&p).map_err(|e| format!("删除符号链接失败: {e}"))?;
            } else {
                fs::remove_file(&p).map_err(|e| format!("删除符号链接失败: {e}"))?;
            }
        }
        #[cfg(not(windows))]
        {
            fs::remove_file(&p).map_err(|e| format!("删除符号链接失败: {e}"))?;
        }
    } else if meta.is_dir() {
        let empty = fs::read_dir(&p)
            .map_err(|e| e.to_string())?
            .next()
            .is_none();
        if !empty && !recursive {
            return Err("目录非空，需要 recursive=true".into());
        }
        fs::remove_dir_all(&p).map_err(|e| format!("删除目录失败: {e}"))?;
    } else {
        fs::remove_file(&p).map_err(|e| format!("删除文件失败: {e}"))?;
    }
    Ok(json!({ "path": path, "deleted": true }))
}

pub fn stat(input: &Value) -> Result<Value, String> {
    let path = get_path(input, "path")?;
    sandbox::check_path(&path, PathOp::Read)?;
    let meta = fs::metadata(&path).map_err(|e| format!("获取信息失败 {path}: {e}"))?;
    Ok(json!({
        "path": path,
        "exists": true,
        "isDir": meta.is_dir(),
        "size": if meta.is_dir() { 0 } else { meta.len() },
        "modified": mtime_to_ms(&meta),
        "readonly": meta.permissions().readonly(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_regex() {
        let re = regex::Regex::new(&glob_to_regex("*.pdf")).unwrap();
        assert!(re.is_match("a.pdf"));
        assert!(re.is_match("A.PDF"));
        assert!(!re.is_match("a.pdfx"));
    }

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zdesk-{name}-{}-{}",
            std::process::id(),
            crate::database::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn read_write_roundtrip() {
        let dir = unique_dir("rw");
        let file = dir.join("a.txt");
        std::fs::write(&file, "hello z").unwrap();
        let out = read(&serde_json::json!({"path": file.to_string_lossy()})).unwrap();
        assert_eq!(out["content"], "hello z");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_requires_content() {
        let dir = unique_dir("write");
        let file = dir.join("a.txt");
        assert!(
            write(&serde_json::json!({ "path": file.to_string_lossy() })).is_err(),
            "缺少 content 必须报错，而不是清空文件"
        );
        write(&serde_json::json!({ "path": file.to_string_lossy(), "content": "ok" })).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "ok");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copy_rejects_destination_inside_source() {
        let dir = unique_dir("copy");
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        let nested = src.join("sub");
        let err = copy(&serde_json::json!({
            "from": src.to_string_lossy(),
            "to": nested.to_string_lossy(),
        }));
        assert!(err.is_err(), "复制到源目录内部应被拒绝");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rename_rejects_dot_and_empty_names() {
        let dir = unique_dir("rename");
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();
        for bad in [".", "..", "   "] {
            assert!(
                rename(&serde_json::json!({
                    "path": file.to_string_lossy(),
                    "new_name": bad,
                }))
                .is_err(),
                "new_name={bad:?} 应被拒绝"
            );
        }
        assert!(file.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
