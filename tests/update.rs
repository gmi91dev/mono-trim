//! `mtrim update` 的自更新端到端测试：用本地 HTTP 服务器模拟 GitHub Release。

mod common;

use common::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
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

/// 极简静态文件服务器，并记录被请求的路径
fn serve(root: PathBuf) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);

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
            recorded.lock().unwrap().push(target.clone());

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

    (format!("http://{addr}"), requests)
}

struct ReleaseOptions {
    /// `version.txt` 的内容；None 表示不提供该文件
    version: Option<String>,
    /// 校验和是否故意写错
    bad_checksum: bool,
    /// 是否提供二进制产物
    include_asset: bool,
}

impl Default for ReleaseOptions {
    fn default() -> Self {
        Self {
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            bad_checksum: false,
            include_asset: true,
        }
    }
}

fn build_release(dir: &TempDir, options: ReleaseOptions) -> PathBuf {
    let root = dir.path().join("mirror");
    let download = root.join("releases/latest/download");
    fs::create_dir_all(&download).unwrap();

    if let Some(version) = options.version {
        fs::write(download.join("version.txt"), format!("{version}\n")).unwrap();
    }

    if options.include_asset {
        let asset = asset_name();
        fs::copy(BIN, download.join(&asset)).unwrap();
        let hash = if options.bad_checksum {
            "0".repeat(64)
        } else {
            sha256_hex(&download.join(&asset))
        };
        fs::write(
            download.join("sha256sums.txt"),
            format!("{hash}  {asset}\n"),
        )
        .unwrap();
    }

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

fn requests_contain(requests: &Arc<Mutex<Vec<String>>>, needle: &str) -> bool {
    requests
        .lock()
        .unwrap()
        .iter()
        .any(|path| path.contains(needle))
}

#[test]
fn updates_do_not_download_assets_when_already_latest() {
    let dir = TempDir::new("update-latest");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    let (base, requests) = serve(build_release(&dir, ReleaseOptions::default()));

    let output = run_update(&dir, &base);

    assert_eq!(exit_code(&output), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("已是最新"));
    assert!(requests_contain(&requests, "version.txt"), "应查询版本文件");
    assert!(
        !requests_contain(&requests, &asset_name()),
        "已是最新时不应下载二进制：{:?}",
        requests.lock().unwrap()
    );
}

#[test]
fn update_rejects_bad_checksum_before_replacing() {
    let dir = TempDir::new("update-bad-sum");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    let (base, requests) = serve(build_release(
        &dir,
        ReleaseOptions {
            version: Some("99.0.0".to_string()),
            bad_checksum: true,
            include_asset: true,
        },
    ));

    let output = run_update(&dir, &base);

    assert!(
        requests_contain(&requests, &asset_name()),
        "发现新版本时应下载"
    );
    let text = stdout(&output);
    assert!(text.contains("sha256 校验失败"), "{text}");
    assert_eq!(exit_code(&output), Some(0), "校验失败只应告警");
}

#[test]
fn update_reports_missing_version_file() {
    let dir = TempDir::new("update-no-version");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    let (base, requests) = serve(build_release(
        &dir,
        ReleaseOptions {
            version: None,
            ..ReleaseOptions::default()
        },
    ));

    let output = run_update(&dir, &base);

    assert_eq!(exit_code(&output), Some(0));
    assert!(
        stdout(&output).contains("检查系统 CLI 更新失败"),
        "{}",
        stdout(&output)
    );
    assert!(
        !requests_contain(&requests, &asset_name()),
        "版本查询失败时不应下载"
    );
}

#[test]
fn update_reports_missing_asset_without_replacing() {
    let dir = TempDir::new("update-no-asset");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    let (base, requests) = serve(build_release(
        &dir,
        ReleaseOptions {
            version: Some("99.0.0".to_string()),
            bad_checksum: false,
            include_asset: false,
        },
    ));

    let output = run_update(&dir, &base);

    assert_eq!(exit_code(&output), Some(0));
    assert!(
        stdout(&output).contains("下载新版本失败"),
        "{}",
        stdout(&output)
    );
    // 缺少产物时，先尝试下载校验和文件就会失败
    assert!(requests_contain(&requests, "sha256sums.txt"));
    assert!(!requests_contain(&requests, &asset_name()));
}
