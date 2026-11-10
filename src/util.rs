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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn write_file_creates_and_overwrites() {
        let dir = TempDir::new("util-write");
        let path = dir.path().join("a.txt");

        write_file(&path, "one").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one");

        write_file(&path, "two").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
    }

    #[test]
    fn write_file_reports_error() {
        let dir = TempDir::new("util-write-err");
        let err = write_file(&dir.path().join("missing/child.txt"), "x").unwrap_err();
        assert!(err.contains("写入"), "错误信息应说明写入失败：{err}");
    }

    #[test]
    fn dedupe_keeps_first_seen_order() {
        let mut items: Vec<String> = ["b", "a", "b", "c", "a"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        dedupe(&mut items);
        assert_eq!(items, ["b", "a", "c"]);
    }

    #[cfg(unix)]
    #[test]
    fn make_executable_sets_exec_bits() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new("util-exec");
        let path = dir.path().join("mtrim");
        write_file(&path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        make_executable(&path);

        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "应补上可执行位");
    }
}
