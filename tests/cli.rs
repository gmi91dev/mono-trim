//! CLI 契约测试：帮助输出、退出码、init/doctor/doc/reset 的端到端行为。

mod common;

use common::*;

#[test]
fn help_lists_all_builtin_commands() {
    let dir = TempDir::new("help");
    let output = mtrim(dir.path(), &["--help"]);

    assert_eq!(exit_code(&output), Some(0));
    let text = stdout(&output);
    for command in ["init", "reapply", "doc", "doctor", "reset"] {
        assert!(text.contains(command), "帮助应列出 {command}：\n{text}");
    }
    assert!(text.contains("mtrim"), "Usage 应显示 mtrim：\n{text}");
}

#[test]
fn init_help_documents_platform_filters() {
    let dir = TempDir::new("init-help");
    let output = mtrim(dir.path(), &["init", "--help"]);

    assert_eq!(exit_code(&output), Some(0));
    let text = stdout(&output);
    for expected in ["--plat", "--exc", "win", "mac", "linux"] {
        assert!(
            text.contains(expected),
            "init 帮助应包含 {expected}：\n{text}"
        );
    }
}

#[test]
fn init_creates_config_and_host_binary() {
    let dir = TempDir::new("init-run");
    let output = mtrim(dir.path(), &["init"]);

    assert!(output.status.success(), "init 应成功：{}", stderr(&output));
    assert!(dir.exists(CONFIG_PATH), "应生成配置文件");
    assert!(
        dir.exists(&format!(".mono-trim/{}", host_binary())),
        "应部署本机产物，实际：{:?}",
        list_files(dir.path())
    );
    assert!(stdout(&output).contains("初始化成功"));
    assert!(stdout(&output).contains("mtrim doctor"), "应提示先自检");
}

#[test]
fn init_keeps_existing_configuration() {
    let dir = TempDir::new("init-idempotent");

    assert!(mtrim(dir.path(), &["init"]).status.success());
    let first = dir.read(CONFIG_PATH);

    let output = mtrim(dir.path(), &["init"]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("保留原有配置"));
    assert_eq!(dir.read(CONFIG_PATH), first);
}

#[test]
fn init_excluding_every_platform_deploys_no_binary() {
    let dir = TempDir::new("init-exclude");

    let output = mtrim(dir.path(), &["init", "-e", "win", "mac", "linux"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("所有平台均被排除"));
    assert!(dir.exists(CONFIG_PATH));
    let binaries: Vec<String> = list_files(&dir.path().join(".mono-trim"))
        .into_iter()
        .filter(|name| name != "config.toml")
        .collect();
    assert!(binaries.is_empty(), "不应部署任何产物：{binaries:?}");
}

#[test]
fn doctor_passes_on_fresh_config() {
    let dir = TempDir::new("doctor-ok");
    init_project(dir.path(), &[("Makefile", "all:\n")]);

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("健康检查通过"));
    assert!(stdout(&output).contains("内置保留命令"));
}

#[test]
fn doctor_fails_on_reserved_profile_name() {
    let dir = TempDir::new("doctor-reserved");
    init_project(dir.path(), &[("Makefile", "all:\n")]);
    append_file(
        dir.path(),
        CONFIG_PATH,
        r#"
[profiles.reset]
mode = "black"
desc = "占用内置命令"
targets = []
"#,
    );

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(
        stdout(&output).contains("内置命令重名"),
        "应指出命名冲突：\n{}",
        stdout(&output)
    );
    assert!(stderr(&output).contains("检查未通过"));
}

#[test]
fn unknown_command_is_rejected() {
    let dir = TempDir::new("unknown-cmd");
    init_project(dir.path(), &[("Makefile", "all:\n")]);

    let output = mtrim(dir.path(), &["nope"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(
        stderr(&output).contains("未知命令 'nope'"),
        "stderr={} stdout={}",
        stderr(&output),
        stdout(&output)
    );
}

#[test]
fn running_outside_git_repo_is_rejected() {
    let dir = TempDir::new("no-repo");

    let output = mtrim(dir.path(), &["doctor"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("不是一个 Git 仓库"));
}

#[test]
fn missing_subcommand_is_rejected() {
    let dir = TempDir::new("no-subcommand");
    init_repo(dir.path(), &[("Makefile", "all:\n")]);

    let output = mtrim(dir.path(), &[]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("未指定具体操作"));
}

#[test]
fn version_flag_is_not_supported() {
    let dir = TempDir::new("version");
    let output = mtrim(dir.path(), &["--version"]);

    assert_eq!(exit_code(&output), Some(2), "clap 参数错误应返回 2");
    assert!(stderr(&output).contains("unexpected argument"));
}

#[test]
fn doc_writes_readme_at_project_root_and_is_idempotent() {
    let dir = TempDir::new("doc-run");
    init_project(dir.path(), &[("README.md", "# 项目说明\n")]);
    append_file(dir.path(), CONFIG_PATH, PROFILES);

    let first = mtrim(dir.path(), &["doc"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let content = dir.read("README.md");
    assert!(content.starts_with("# 项目说明"), "应保留原有内容");
    assert_eq!(content.matches("<!-- mono-trim:begin -->").count(), 1);
    assert!(content.contains("./.mono-trim/mtrim win-dev"));
    assert!(content.contains("./.mono-trim/mtrim reset"));
    let second = mtrim(dir.path(), &["doc"]);
    assert!(second.status.success());
    assert_eq!(dir.read("README.md"), content, "重复执行不应产生重复区块");
}

#[test]
fn reset_is_noop_when_workspace_is_full() {
    let dir = TempDir::new("reset-full");
    init_project(dir.path(), &[("Makefile", "all:\n")]);

    let output = mtrim(dir.path(), &["reset"]);

    assert_eq!(exit_code(&output), Some(0));
    assert!(stdout(&output).contains("本就是全量状态"));
}

#[test]
fn reapply_requires_sparse_checkout() {
    let dir = TempDir::new("reapply-off");
    init_project(dir.path(), &[("Makefile", "all:\n")]);

    let output = mtrim(dir.path(), &["reapply"]);

    assert_eq!(exit_code(&output), Some(1));
    assert!(stderr(&output).contains("尚未启用稀疏检出"));
}
