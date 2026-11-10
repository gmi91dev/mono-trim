//! 文件与集合相关的小工具。

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::Result;

pub fn write_file(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content).map_err(|e| format!("写入 {} 失败：{e}", path.display()))
}

#[cfg(unix)]
pub fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        let _ = fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
pub fn make_executable(_path: &Path) {}

/// 原地去重（保留首次出现的顺序）
pub fn dedupe(items: &mut Vec<String>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.clone()));
}
