//! `mtrim update` 的自更新端到端测试：用本地 HTTP 服务器模拟 GitHub Release。

mod common;

use common::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;

fn asset_name() -> String {
    let os = match std::env::consts::OS {
        "windows" => "win",
        "macos" => "macos",
        other => other,
    };
    let ext = if cfg!(windows) { ".exe" } else { "" };
    format!("mtrim-{os}-{}{ext}", std::env::consts::ARCH)
}

fn sha256_hex(path: &Path) -> String {
    let bytes = fs::read(path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 启动最小静态文件服务器（只处理 GET），返回 base URL
fn serve(root: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0u8; 8192];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let target = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/")
                .trim_start_matches('/')
                .to_string();

            match fs::read(root.join(target)) {
                Ok(body) => {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&body);
                }
                Err(_) => {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            }
        }
    });

    format!("http://{addr}")
}

/// 生成 `mirror/releases/latest/download/` 目录；checksum 为 None 时使用真实校验和
fn build_release(dir: &TempDir, checksum: Option<&str>) -> PathBuf {
    let root = dir.path().join("mirror");
    let download = root.join("releases/latest/download");
    fs::create_dir_all(&download).unwrap();

    let asset = asset_name();
    fs::copy(BIN, download.join(&asset)).unwrap();
    let hash = checksum
        .map(str::to_string)
        .unwrap_or_else(|| sha256_hex(&download.join(&asset)));
    fs::write(
        download.join("sha256sums.txt"),
        format!("{hash}  {asset}\n"),
    )
    .unwrap();

    root
}

fn run_update(dir: &TempDir, base: &str) -> Output {
    Command::new(BIN)
        .arg("update")
        .current_dir(dir.path())
        .env("MONO_TRIM_MIRROR", base)
        .output()
        .expect("执行 mtrim update 失败")
}

#[test]
fn update_downloads_and_verifies_release() {
    let dir = TempDir::new("update-verify");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    let base = serve(build_release(&dir, None));

    let output = run_update(&dir, &base);

    assert_eq!(exit_code(&output), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(!text.contains("校验失败"), "{text}");
    assert!(text.contains("已是最新"), "应识别出与当前版本相同：{text}");
}

#[test]
fn update_rejects_bad_checksum() {
    let dir = TempDir::new("update-bad-sum");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    let base = serve(build_release(&dir, Some(&"0".repeat(64))));

    let output = run_update(&dir, &base);

    let text = stdout(&output);
    assert!(text.contains("sha256 校验失败"), "{text}");
    assert_eq!(
        exit_code(&output),
        Some(0),
        "校验失败只应告警，不应中断项目配置同步"
    );
}

#[test]
fn update_reports_download_failure() {
    let dir = TempDir::new("update-404");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    let empty = dir.path().join("empty-mirror");
    fs::create_dir_all(&empty).unwrap();
    let base = serve(empty);

    let output = run_update(&dir, &base);

    assert_eq!(exit_code(&output), Some(0));
    let text = stdout(&output);
    assert!(text.contains("检查系统 CLI 更新失败"), "{text}");
}
