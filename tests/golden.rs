//! 文本快照测试：锁定 `--help`、配置模板与 `doc` 速查表的精确内容。
//!
//! 快照由 `INSTA_UPDATE=always cargo test --test golden` 生成，
//! 日常 `cargo test` 只做比对；CI 中不允许自动更新。
//! 通过 `filters` 把 `mtrim.exe` 归一化为 `mtrim`，保证三端快照一致。

mod common;

use common::*;

#[test]
fn help_output() {
    let dir = TempDir::new("golden-help");
    let output = mtrim(dir.path(), &["--help"]);
    assert!(output.status.success(), "{}", stderr(&output));

    insta::with_settings!({filters => vec![(r"mtrim\.exe", "mtrim")]}, {
        insta::assert_snapshot!("help", stdout(&output));
    });
}

#[test]
fn init_help_output() {
    let dir = TempDir::new("golden-init-help");
    let output = mtrim(dir.path(), &["init", "--help"]);
    assert!(output.status.success(), "{}", stderr(&output));

    insta::with_settings!({filters => vec![(r"mtrim\.exe", "mtrim")]}, {
        insta::assert_snapshot!("init_help", stdout(&output));
    });
}

#[test]
fn generated_config_template() {
    let dir = TempDir::new("golden-config");
    // 排除全部平台，避免把被测二进制复制进临时目录
    let output = mtrim(dir.path(), &["init", "-e", "win", "mac", "linux"]);
    assert!(output.status.success(), "{}", stderr(&output));

    insta::assert_snapshot!("config_template", dir.read(CONFIG_PATH));
}

#[test]
fn doc_table_output() {
    let dir = TempDir::new("golden-doc");
    init_repo(dir.path(), &[("README.md", "# 项目说明\n")]);
    assert!(
        mtrim(dir.path(), &["init", "-e", "win", "mac", "linux"])
            .status
            .success()
    );
    append_file(dir.path(), CONFIG_PATH, PROFILES);

    let output = mtrim(dir.path(), &["doc"]);
    assert!(output.status.success(), "{}", stderr(&output));

    insta::assert_snapshot!("doc_table", dir.read("README.md"));
}
