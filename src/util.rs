//! 文件与集合相关的小工具。

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::Result;

/// 计算文件的 sha256（小写十六进制）
pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

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
    fn sha256_matches_known_vector() {
        let dir = TempDir::new("util-sha256");
        let path = dir.path().join("abc.txt");
        write_file(&path, "abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_reports_missing_file() {
        assert!(sha256_file(Path::new("does-not-exist")).is_err());
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
