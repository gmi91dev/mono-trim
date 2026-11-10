//! `mtrim clone` 的端到端测试：只检出配置文件，再按 profile 拉取所需文件。

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
            "profiles",
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

/// 源仓库放在独立临时目录，工作目录另建，避免默认目录名冲突
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

fn expected_white_files() -> Vec<String> {
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
fn clone_checks_out_only_config_files() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-only");

    let output = mtrim(work.path(), &["clone", &repo.to_string_lossy()]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("仅配置文件"));
    let mut expected = vec![SYSTEM_CONFIG.to_string(), USER_CONFIG.to_string()];
    expected.sort();
    assert_eq!(list_files(&work.path().join("repo")), expected);
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
fn clone_with_profile_materializes_files_immediately() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-profile");

    let output = mtrim(
        work.path(),
        &[
            "clone",
            &repo.to_string_lossy(),
            "target",
            "-p",
            "rust-only",
        ],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        list_files(&work.path().join("target")),
        expected_white_files()
    );
}

#[test]
fn clone_then_profile_pulls_required_files() {
    if skip_without_git() {
        return;
    }
    let (_source, repo, work) = fixture("clone-then-profile");
    assert!(
        mtrim(work.path(), &["clone", &repo.to_string_lossy(), "copy"])
            .status
            .success()
    );

    let target = work.path().join("copy");
    let output = mtrim(&target, &["rust-only"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(list_files(&target), expected_white_files());
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
fn clone_of_repo_without_config_warns() {
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
    assert!(
        stdout(&output).contains("未使用 mono-trim"),
        "{}",
        stdout(&output)
    );
}
