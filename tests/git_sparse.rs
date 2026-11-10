//! 真实 Git 稀疏检出的端到端测试：黑名单 / 白名单 / applyall 的物理文件结果。

mod common;

use common::*;

const PROJECT_FILES: &[(&str, &str)] = &[
    ("Makefile", "all:\n"),
    ("rust/src/main.rs", "fn main() {}\n"),
    ("apps/ios/App.swift", "ios\n"),
    ("apps/android/Main.kt", "android\n"),
    ("assets/large_video.mp4", "binary\n"),
];

fn skip_without_git() -> bool {
    if git_available() {
        false
    } else {
        eprintln!("跳过：当前环境未找到 git");
        true
    }
}

fn setup(label: &str) -> TempDir {
    let dir = TempDir::new(label);
    init_project(dir.path(), PROJECT_FILES);
    append_file(dir.path(), USER_CONFIG, PROFILES);
    dir
}

#[test]
fn black_mode_removes_excluded_paths() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-black");

    let output = mtrim(dir.path(), &["win-dev"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("黑名单排除"));

    let files = list_files(dir.path());
    assert!(
        !files.contains(&"apps/ios/App.swift".to_string()),
        "iOS 壳应被物理移除：{files:?}"
    );
    assert!(
        !files.contains(&"assets/large_video.mp4".to_string()),
        "大资产应被物理移除：{files:?}"
    );
    assert!(files.contains(&"apps/android/Main.kt".to_string()));
    assert!(files.contains(&"rust/src/main.rs".to_string()));
    assert!(
        files.contains(&SYSTEM_CONFIG.to_string()),
        "配置文件必须保留：{files:?}"
    );
}

#[test]
fn white_mode_keeps_only_whitelist() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-white");

    let output = mtrim(dir.path(), &["rust-only"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("白名单仅保留"));

    let mut expected = vec![
        "Makefile".to_string(),
        "rust/src/main.rs".to_string(),
        SYSTEM_CONFIG.to_string(),
        USER_CONFIG.to_string(),
    ];
    expected.sort();

    assert_eq!(list_files(dir.path()), expected);
}

#[test]
fn applyall_restores_full_workspace() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-applyall");
    assert!(mtrim(dir.path(), &["rust-only"]).status.success());

    let output = mtrim(dir.path(), &["applyall"]);
    assert!(output.status.success(), "{}", stderr(&output));

    let mut expected: Vec<String> = PROJECT_FILES
        .iter()
        .map(|(path, _)| path.to_string())
        .collect();
    expected.push(SYSTEM_CONFIG.to_string());
    expected.push(USER_CONFIG.to_string());
    expected.sort();

    assert_eq!(list_files(dir.path()), expected);
}

#[test]
fn switching_profiles_refreshes_sparse_rules() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-switch");
    assert!(mtrim(dir.path(), &["rust-only"]).status.success());

    let output = mtrim(dir.path(), &["win-dev"]);
    assert!(output.status.success(), "{}", stderr(&output));

    let files = list_files(dir.path());
    assert!(
        files.contains(&"apps/android/Main.kt".to_string()),
        "切回黑名单后应恢复其它平台：{files:?}"
    );
    assert!(!files.contains(&"apps/ios/App.swift".to_string()));
}

#[test]
fn applyall_is_idempotent() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-applyall-twice");
    assert!(mtrim(dir.path(), &["win-dev"]).status.success());
    assert!(mtrim(dir.path(), &["applyall"]).status.success());
    let once = list_files(dir.path());

    assert!(mtrim(dir.path(), &["applyall"]).status.success());

    assert_eq!(list_files(dir.path()), once);
}
