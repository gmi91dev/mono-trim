//! CLI 契约测试：帮助、版本、退出码，以及各内置命令的端到端行为。

mod common;

use common::*;

#[test]
fn help_lists_all_builtin_commands() {
    let dir = TempDir::new("help");
    let output = mtrim(dir.path(), &["--help"]);

    assert_eq!(exit_code(&output), Some(0));
    let text = stdout(&output);
    for command in [
        "init", "clone", "update", "doctor", "ls", "show", "doc", "applyall",
    ] {
        assert!(text.contains(command), "帮助应列出 {command}：\n{text}");
    }
}

#[test]
fn version_flag_prints_current_version() {
    let dir = TempDir::new("version");
    let output = mtrim(dir.path(), &["--version"]);

    assert_eq!(exit_code(&output), Some(0));
    let text = stdout(&output);
    assert!(text.starts_with("mtrim "), "{text}");
    assert_eq!(
        text.split_whitespace().nth(1).unwrap(),
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn init_creates_both_config_files() {
    let dir = TempDir::new("init");
    let output = mtrim(dir.path(), &["init"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(dir.exists(SYSTEM_CONFIG), "应生成系统配置");
    assert!(dir.exists(USER_CONFIG), "应生成用户配置");
    assert!(stdout(&output).contains("已初始化项目配置"));
}

#[test]
fn init_rewrites_system_config_but_keeps_user_config() {
    let dir = TempDir::new("init-rerun");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    append_file(
        dir.path(),
        USER_CONFIG,
        "\n[profiles.keep]\nmode = \"black\"\ntargets = []\n",
    );
    dir.write(SYSTEM_CONFIG, "version = \"0.0.1\"\n");

    let output = mtrim(dir.path(), &["init"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(dir.read(SYSTEM_CONFIG).contains(env!("CARGO_PKG_VERSION")));
    assert!(dir.read(USER_CONFIG).contains("profiles.keep"));
    assert!(stdout(&output).contains("已存在，保持不动"));
}

#[test]
fn doctor_passes_on_fresh_config() {
    let dir = TempDir::new("doctor-ok");
    assert!(mtrim(dir.path(), &["init"]).status.success());

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("配置检查通过"));
}

#[test]
fn doctor_fails_on_reserved_profile_name() {
    let dir = TempDir::new("doctor-reserved");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    append_file(
        dir.path(),
        USER_CONFIG,
        "\n[profiles.ls]\nmode = \"black\"\ndesc = \"占用内置命令\"\ntargets = []\n",
    );

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(
        stdout(&output).contains("内置命令重名"),
        "{}",
        stdout(&output)
    );
    assert!(stderr(&output).contains("检查未通过"));
}

#[test]
fn doctor_fails_on_broken_toml() {
    let dir = TempDir::new("doctor-broken");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    dir.write(USER_CONFIG, "mode = ");

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("检查未通过"));
}

#[test]
fn doctor_fails_when_not_initialized() {
    let dir = TempDir::new("doctor-none");

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("mtrim init"));
}

#[test]
fn ls_lists_user_commands_and_all_includes_builtins() {
    let dir = TempDir::new("ls");
    assert!(mtrim(dir.path(), &["init"]).status.success());
    append_file(dir.path(), USER_CONFIG, PROFILES);

    let user_only = mtrim(dir.path(), &["ls"]);
    assert_eq!(exit_code(&user_only), Some(0));
    let text = stdout(&user_only);
    assert!(text.contains("win-dev"), "{text}");
    assert!(text.contains("rust-only"), "{text}");
    assert!(!text.contains("applyall"), "默认不应列出内置命令：{text}");

    let all = mtrim(dir.path(), &["ls", "-a"]);
    let text = stdout(&all);
    for command in [
        "init", "clone", "update", "doctor", "ls", "show", "doc", "applyall",
    ] {
        assert!(text.contains(command), "ls -a 应列出 {command}：{text}");
    }
}

#[test]
fn ls_mentions_init_when_nothing_configured() {
    let dir = TempDir::new("ls-empty");
    let output = mtrim(dir.path(), &["ls"]);

    assert_eq!(exit_code(&output), Some(0));
    assert!(stdout(&output).contains("mtrim init"));
}

#[test]
fn show_prints_config_paths() {
    let dir = TempDir::new("show");
    assert!(mtrim(dir.path(), &["init"]).status.success());

    let output = mtrim(dir.path(), &["show"]);

    assert_eq!(exit_code(&output), Some(0));
    let text = stdout(&output);
    assert!(text.contains(SYSTEM_CONFIG), "{text}");
    assert!(text.contains(USER_CONFIG), "{text}");
    assert!(text.contains("存在"));
}

#[test]
fn doc_writes_markdown_table_to_project_root() {
    let dir = TempDir::new("doc");
    init_project(dir.path(), &[("README.md", "# 项目说明\n")]);
    append_file(dir.path(), USER_CONFIG, PROFILES);

    let first = mtrim(dir.path(), &["doc"]);
    assert!(first.status.success(), "{}", stderr(&first));

    let content = dir.read("README.md");
    assert!(content.starts_with("# 项目说明"), "应保留原有内容");
    assert_eq!(content.matches("<!-- mono-trim:begin -->").count(), 1);
    assert!(content.contains("mtrim win-dev"), "{content}");
    assert!(content.contains("mtrim applyall"), "{content}");

    let second = mtrim(dir.path(), &["doc"]);
    assert!(second.status.success());
    assert_eq!(dir.read("README.md"), content, "重复执行不应产生重复区块");
}

#[test]
fn doc_requires_initialized_project() {
    let dir = TempDir::new("doc-uninitialized");

    let output = mtrim(dir.path(), &["doc"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("mtrim init"));
}

#[test]
fn unknown_command_points_to_ls() {
    let dir = TempDir::new("unknown");
    assert!(mtrim(dir.path(), &["init"]).status.success());

    let output = mtrim(dir.path(), &["nope"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("未知命令 'nope'"));
    assert!(stderr(&output).contains("mtrim ls"));
}

#[test]
fn missing_subcommand_is_rejected() {
    let dir = TempDir::new("no-subcommand");

    let output = mtrim(dir.path(), &[]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("未指定具体操作"));
}

#[test]
fn applyall_requires_git_repository() {
    let dir = TempDir::new("applyall-norepo");

    let output = mtrim(dir.path(), &["applyall"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("不是一个 Git 仓库"));
}

#[test]
fn init_warns_when_config_is_gitignored() {
    let dir = TempDir::new("init-ignored");
    init_repo(
        dir.path(),
        &[
            (
                "Makefile", "all:
",
            ),
            (
                ".gitignore",
                ".mono-trim/
mono-trim.toml
",
            ),
        ],
    );

    let output = mtrim(dir.path(), &["init"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("被 .gitignore 忽略"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn doctor_reports_config_ignored_by_gitignore() {
    let dir = TempDir::new("doctor-ignored");
    init_project(
        dir.path(),
        &[(
            "Makefile", "all:
",
        )],
    );
    dir.write(
        ".gitignore",
        ".mono-trim/
mono-trim.toml
",
    );

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(
        stdout(&output).contains("被 .gitignore 忽略"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn doctor_passes_when_gitignore_negates_config() {
    let dir = TempDir::new("doctor-negated");
    init_project(
        dir.path(),
        &[(
            "Makefile", "all:
",
        )],
    );
    dir.write(
        ".gitignore",
        "*.log
!.mono-trim/
!mono-trim.toml
",
    );

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(0), "{}", stdout(&output));
    assert!(!stdout(&output).contains("被 .gitignore 忽略"));
}
