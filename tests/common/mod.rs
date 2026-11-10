//! 集成测试共用工具：临时目录、CLI 调用封装、真实 Git 仓库搭建。
//!
//! 每个集成测试文件都会独立编译本模块，未在该文件用到的辅助函数并非真的死代码
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 由 Cargo 注入的二进制路径
pub const BIN: &str = env!("CARGO_BIN_EXE_mtrim");

/// 系统配置的相对路径
pub const SYSTEM_CONFIG: &str = ".mono-trim/config.toml";
/// 用户配置的相对路径
pub const USER_CONFIG: &str = "mono-trim.toml";

/// 集成测试共用的一组用户配置
pub const PROFILES: &str = r#"
[profiles.win-dev]
mode = "black"
desc = "隐藏 iOS 壳与大资产"
targets = ["apps/ios/", "assets/large_video.mp4"]

[profiles.rust-only]
mode = "white"
desc = "仅保留 Rust 核心层"
targets = ["rust/", "Makefile"]
"#;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 进程内唯一的临时目录，`Drop` 时自动清理
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mono-trim-it-{}-{label}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("创建测试临时目录失败");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, relative: &str, content: &str) -> PathBuf {
        write_file(&self.path, relative, content)
    }

    pub fn read(&self, relative: &str) -> String {
        read_file(&self.path, relative)
    }

    pub fn exists(&self, relative: &str) -> bool {
        exists(&self.path, relative)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn write_file(dir: &Path, relative: &str, content: &str) -> PathBuf {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
    path
}

pub fn read_file(dir: &Path, relative: &str) -> String {
    fs::read_to_string(dir.join(relative)).unwrap()
}

pub fn exists(dir: &Path, relative: &str) -> bool {
    dir.join(relative).exists()
}

pub fn append_file(dir: &Path, relative: &str, content: &str) {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(dir.join(relative))
        .unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

/// 递归列出目录下的相对文件路径（统一 `/` 分隔，跳过 `.git`）
pub fn list_files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_name() == ".git" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    files
}

/// 在指定目录执行被测二进制
pub fn mtrim(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("执行 mtrim 失败")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn exit_code(output: &Output) -> Option<i32> {
    output.status.code()
}

pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("执行 git 失败")
}

/// 建一个带初始提交的仓库（提交身份用 -c 临时指定，避免依赖全局配置）
pub fn init_repo(dir: &Path, files: &[(&str, &str)]) {
    assert!(git(dir, &["init", "-q"]).status.success(), "git init 失败");
    for (path, content) in files {
        write_file(dir, path, content);
    }
    assert!(git(dir, &["add", "-A"]).status.success(), "git add 失败");
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=mono-trim-test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "init",
        ])
        .current_dir(dir)
        .output()
        .expect("执行 git commit 失败");
    assert!(
        output.status.success(),
        "git commit 失败：{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 建仓库并完成 `mtrim init`
pub fn init_project(dir: &Path, files: &[(&str, &str)]) {
    init_repo(dir, files);
    let output = mtrim(dir, &["init"]);
    assert!(output.status.success(), "init 失败：{}", stderr(&output));
}
