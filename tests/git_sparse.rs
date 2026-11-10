//! 真实 Git 稀疏检出的端到端测试：黑名单 / 白名单 / reset 的物理文件结果。

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
    append_file(
        dir.path(),
        CONFIG_PATH,
        r#"
[profiles.win-dev]
mode = "black"
desc = "隐藏 iOS 壳与大资产"
targets = ["apps/ios/", "assets/large_video.mp4"]

[profiles.rust-only]
mode = "white"
desc = "仅保留 Rust 核心层"
targets = ["rust/", "Makefile"]
"#,
    );
    dir
}

fn tool_files() -> Vec<String> {
    vec![
        CONFIG_PATH.to_string(),
        format!(".mono-trim/{}", host_binary()),
    ]
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
        files.contains(&CONFIG_PATH.to_string()),
        "工具自身必须保留：{files:?}"
    );
}

#[test]
fn white_mode_keeps_only_whitelist_and_tool_dir() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-white");

    let output = mtrim(dir.path(), &["rust-only"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("白名单仅保留"));

    let mut expected = tool_files();
    expected.extend(["Makefile".to_string(), "rust/src/main.rs".to_string()]);
    expected.sort();

    assert_eq!(list_files(dir.path()), expected);
}

#[test]
fn reset_restores_full_workspace() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-reset");
    assert!(mtrim(dir.path(), &["rust-only"]).status.success());

    let output = mtrim(dir.path(), &["reset"]);
    assert!(output.status.success(), "{}", stderr(&output));

    let mut expected: Vec<String> = PROJECT_FILES
        .iter()
        .map(|(path, _)| path.to_string())
        .collect();
    expected.extend(tool_files());
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
fn reapply_refreshes_existing_sparse_rules() {
    if skip_without_git() {
        return;
    }
    let dir = setup("sparse-reapply");
    assert!(mtrim(dir.path(), &["win-dev"]).status.success());
    let before = list_files(dir.path());

    let output = mtrim(dir.path(), &["reapply"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(list_files(dir.path()), before);
}
