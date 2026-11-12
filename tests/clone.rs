//! `mtrim clone` 的端到端测试：标准克隆 + mono-trim 配置识别 + 可选立即执行命令。

mod common;

use common::*;
use std::path::PathBuf;

const PROJECT_FILES: &[(&str, &str)] = &[
    ("Makefile", "all:\n"),
    ("rust/src/main.rs", "fn main() {}\n"),
    ("apps/ios/App.swift", "ios\n"),
    ("apps/android/Main.kt", "android\n"),
];

const PROFILES: &str = r#"
[profiles.rust-only]
mode = "white"
desc = "仅保留 Rust 核心层"
targets = ["rust/", "Makefile"]
"#;

fn skip_without_git() -> bool {
    if git_available() {
        false
    } else {
        eprintln!("跳过：当前环境未找到 git");
        true
    }
}

fn commit_all(repo: &std::path::Path) {
    assert!(git(repo, &["add", "-A"]).status.success());
    let output = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=mono-trim-test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "config",
        ])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 源仓库在独立临时目录，工作目录另建，避免默认目录名冲突
fn fixture(label: &str) -> (TempDir, PathBuf, TempDir) {
    let source = TempDir::new(&format!("{label}-src"));
    let repo = source.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    init_project(&repo, PROJECT_FILES);
    append_file(&repo, USER_CONFIG, PROFILES);
    commit_all(&repo);

    let work = TempDir::new(&format!("{label}-work"));
    (source, repo, work)
}

fn all_project_files() -> Vec<String> {
    let mut expected: Vec<String> = PROJECT_FILES
        .iter()
        .map(|(path, _)| path.to_string())
        .collect();
    expected.push(SYSTEM_CONFIG.to_string());
    expected.push(USER_CONFIG.to_string());
    expected.sort();
    expected
}

fn white_profile_files() -> Vec<String> {
    let mut expected = vec![
        SYSTEM_CONFIG.to_string(),
        USER_CONFIG.to_string(),
        "Makefile".to_string(),
        "rust/src/main.rs".to_string(),
    ];
    expected.sort();
    expected
}

#[test]
fn clone_fetches_repository_and_trim_config() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-full");

    let output = mtrim(work.path(), &["clone", &repo.to_string_lossy()]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("可用命令"), "{}", stdout(&output));
    assert_eq!(list_files(&work.path().join("repo")), all_project_files());
}

#[test]
fn clone_uses_repository_name_as_default_directory() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-default-dir");

    let output = mtrim(work.path(), &["clone", &repo.to_string_lossy()]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(work.exists("repo/.mono-trim/config.toml"));
}

#[test]
fn clone_with_command_trims_immediately() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-command");

    let output = mtrim(
        work.path(),
        &[
            "clone",
            &repo.to_string_lossy(),
            "target",
            "-c",
            "rust-only",
        ],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        list_files(&work.path().join("target")),
        white_profile_files()
    );
}

#[test]
fn clone_rejects_unknown_command() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-bad-command");

    let output = mtrim(
        work.path(),
        &["clone", &repo.to_string_lossy(), "target", "-c", "nope"],
    );

    assert_eq!(exit_code(&output), Some(1));
    assert!(
        stderr(&output).contains("配置中没有 'nope' 命令"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn clone_refuses_non_empty_directory() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-non-empty");
    work.write("existing/file.txt", "x");

    let output = mtrim(work.path(), &["clone", &repo.to_string_lossy(), "existing"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(
        stderr(&output).contains("已存在且不为空"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn clone_of_repo_without_config_suggests_init() {
    if skip_without_git() {
        return;
    }
    let source = TempDir::new("clone-no-config-src");
    let repo = source.path().join("plain");
    std::fs::create_dir_all(&repo).unwrap();
    init_repo(&repo, &[("Makefile", "all:\n")]);
    let work = TempDir::new("clone-no-config-work");

    let output = mtrim(
        work.path(),
        &["clone", &repo.to_string_lossy(), "plain-copy"],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("尚未使用 mono-trim 管理"), "{text}");
    assert!(text.contains("mtrim init"), "{text}");
}

#[test]
fn clone_with_command_fails_when_repo_is_unmanaged() {
    if skip_without_git() {
        return;
    }
    let source = TempDir::new("clone-unmanaged-src");
    let repo = source.path().join("plain");
    std::fs::create_dir_all(&repo).unwrap();
    init_repo(&repo, &[("Makefile", "all:\n")]);
    let work = TempDir::new("clone-unmanaged-work");

    let output = mtrim(
        work.path(),
        &["clone", &repo.to_string_lossy(), "copy", "-c", "anything"],
    );

    assert_eq!(exit_code(&output), Some(1));
    assert!(
        stderr(&output).contains("尚未使用 mono-trim 管理"),
        "{}",
        stderr(&output)
    );
}
