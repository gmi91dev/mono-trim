//! 文本快照测试：锁定 `--help`、生成的配置文件与 `doc` 速查表的精确内容。
//!
//! 更新方式：`INSTA_UPDATE=always cargo test --test golden`；
//! CI 中 `INSTA_UPDATE=no`，只比对不更新。

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
fn generated_system_config() {
    let dir = TempDir::new("golden-system-config");
    assert!(mtrim(dir.path(), &["init"]).status.success());

    insta::assert_snapshot!("system_config", dir.read(SYSTEM_CONFIG));
}

#[test]
fn generated_user_config_template() {
    let dir = TempDir::new("golden-user-config");
    assert!(mtrim(dir.path(), &["init"]).status.success());

    insta::assert_snapshot!("user_config", dir.read(USER_CONFIG));
}

#[test]
fn doc_table_output() {
    let dir = TempDir::new("golden-doc");
    init_project(dir.path(), &[("README.md", "# 项目说明\n")]);
    append_file(dir.path(), USER_CONFIG, PROFILES);

    let output = mtrim(dir.path(), &["doc"]);
    assert!(output.status.success(), "{}", stderr(&output));

    insta::assert_snapshot!("doc_table", dir.read("README.md"));
}

#[test]
fn ls_output() {
    let dir = TempDir::new("golden-ls");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    append_file(dir.path(), USER_CONFIG, PROFILES);

    let output = mtrim(dir.path(), &["ls"]);
    assert!(output.status.success());

    insta::assert_snapshot!("ls", stdout(&output));
}
