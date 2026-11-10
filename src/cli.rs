//! 命令行入口定义：仅描述命令/参数，不含业务逻辑。

use clap::{Arg, ArgAction, Command as ClapCommand};

/// 用户实际调用的命令名（与 Cargo 的 [[bin]] 名称保持一致）
pub const COMMAND: &str = "mtrim";

/// 内置子命令：属于系统保留字，`mono-trim.toml` 的 profiles 不可占用
pub const BUILTIN_COMMANDS: [&str; 7] =
    ["init", "update", "doctor", "ls", "show", "doc", "applyall"];

/// 内置命令说明，供 `ls -a` 与文档输出
pub const BUILTIN_DESCRIPTIONS: [(&str, &str); 7] = [
    ("init", "使用内置默认值重新初始化项目配置"),
    ("update", "升级系统 CLI 并同步项目配置版本"),
    ("doctor", "校验系统配置与用户配置"),
    ("ls", "列出可用的裁剪命令"),
    ("show", "打印配置文件路径"),
    ("doc", "将命令速查表写入文档"),
    ("applyall", "恢复项目为全量工作空间"),
];

/// 由 clap 自动生成的命令，同样不可被 profiles 占用
pub const AUTO_COMMAND: &str = "help";

/// 全部保留命令名（内置 + 自动生成），用于配置模板提示与命名校验
pub fn reserved_commands() -> impl Iterator<Item = &'static str> {
    BUILTIN_COMMANDS
        .iter()
        .copied()
        .chain(std::iter::once(AUTO_COMMAND))
}

pub fn is_reserved(name: &str) -> bool {
    reserved_commands().any(|command| command == name)
}

/// 校验 profiles 名称能否真正作为外部子命令被调用
pub fn validate_profile_name(name: &str) -> crate::Result<()> {
    if name.trim().is_empty() {
        return Err("profiles 名称为空，无法作为命令调用。".into());
    }
    if name != name.trim() {
        return Err(format!(
            "profiles 名称 '{name}' 含首尾空白，无法作为命令调用。"
        ));
    }
    if is_reserved(name) {
        return Err(format!(
            "profiles 名称 '{name}' 与内置命令重名，永远不会被执行，请改用其它名称。"
        ));
    }
    if name.starts_with('-') {
        return Err(format!(
            "profiles 名称 '{name}' 不能以 '-' 开头，会被解析为命令行参数。"
        ));
    }
    if name.chars().any(char::is_whitespace) {
        return Err(format!("profiles 名称 '{name}' 不能包含空白字符。"));
    }
    Ok(())
}

pub fn build() -> ClapCommand {
    ClapCommand::new(COMMAND)
        .version(crate::config::current_version())
        .about("高内聚、零污染的项目级多平台工作空间裁剪引擎")
        .subcommand(ClapCommand::new("init").about(description("init")))
        .subcommand(ClapCommand::new("update").about(description("update")))
        .subcommand(ClapCommand::new("doctor").about(description("doctor")))
        .subcommand(
            ClapCommand::new("ls").about(description("ls")).arg(
                Arg::new("all")
                    .short('a')
                    .long("all")
                    .action(ArgAction::SetTrue)
                    .help("同时列出内置命令"),
            ),
        )
        .subcommand(ClapCommand::new("show").about(description("show")))
        .subcommand(ClapCommand::new("doc").about(description("doc")))
        .subcommand(ClapCommand::new("applyall").about(description("applyall")))
        .allow_external_subcommands(true)
}

fn description(command: &str) -> &'static str {
    BUILTIN_DESCRIPTIONS
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, desc)| *desc)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_list_stays_in_sync_with_clap() {
        let mut declared: Vec<String> = build()
            .get_subcommands()
            .map(|command| command.get_name().to_string())
            .filter(|name| name != AUTO_COMMAND)
            .collect();
        declared.sort();

        let mut expected: Vec<String> = BUILTIN_COMMANDS
            .iter()
            .map(|name| name.to_string())
            .collect();
        expected.sort();

        assert_eq!(declared, expected, "BUILTIN_COMMANDS 与 clap 子命令不一致");
    }

    #[test]
    fn every_builtin_has_a_description() {
        for name in BUILTIN_COMMANDS {
            assert!(!description(name).is_empty(), "{name} 缺少说明");
        }
    }

    #[test]
    fn rejects_profile_names_that_cannot_be_invoked() {
        for name in [
            "init", "update", "doctor", "ls", "show", "doc", "applyall", "help",
        ] {
            assert!(validate_profile_name(name).is_err(), "'{name}' 应被拒绝");
        }
        for name in ["", " win-dev", "-x", "my profile"] {
            assert!(validate_profile_name(name).is_err(), "'{name}' 应被拒绝");
        }
        for name in ["win-dev", "rust-only", "android_ui", "iOS"] {
            assert!(validate_profile_name(name).is_ok(), "'{name}' 应被允许");
        }
    }

    #[test]
    fn reserved_list_includes_auto_help() {
        let reserved: Vec<&str> = reserved_commands().collect();
        assert_eq!(reserved.len(), BUILTIN_COMMANDS.len() + 1);
        assert!(reserved.contains(&AUTO_COMMAND));
    }

    #[test]
    fn version_flag_is_supported() {
        let err = build()
            .try_get_matches_from(["mtrim", "--version"])
            .unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        assert!(err.to_string().contains(crate::config::current_version()));
    }
}
