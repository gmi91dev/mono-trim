//! 命令行入口定义：仅描述命令/参数，不含业务逻辑。

use clap::{Arg, Command as ClapCommand};

/// 用户实际调用的命令名（与 Cargo 的 [[bin]] 名称保持一致）
pub const COMMAND: &str = "mtrim";

/// 旧命令名，仅用于兼容已分发的历史产物
pub const LEGACY_COMMAND: &str = "mono-trim";

/// 支持的多平台产物标识，同时作为 clap 的取值校验
pub const PLATFORMS: [&str; 3] = ["win", "mac", "linux"];

/// 内置子命令：属于系统保留字，config.toml 的 profiles 不可占用
pub const BUILTIN_COMMANDS: [&str; 5] = ["init", "reapply", "doc", "doctor", "reset"];
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
        .about("高内聚、零污染的项目级多平台工作空间裁剪引擎")
        .subcommand(
            ClapCommand::new("init")
                .about("在当前项目下初始化环境")
                .arg(
                    Arg::new("platforms")
                        .long("plat")
                        .short('p')
                        .num_args(1..)
                        .value_parser(PLATFORMS)
                        .help(
                            "指定需要初始化的平台 (可选值: win, mac, linux)。不传默认全部初始化。",
                        ),
                )
                .arg(
                    Arg::new("exclude")
                        .long("exc")
                        .short('e')
                        .num_args(1..)
                        .value_parser(PLATFORMS)
                        .help("指定需要排除的平台 (可选值: win, mac, linux)。"),
                ),
        )
        .subcommand(
            ClapCommand::new("reapply").about("根据当前的 Git 稀疏设置重新刷新本地物理硬盘"),
        )
        .subcommand(ClapCommand::new("doc").about("将自定义命令及说明自动导入到指定的文档中"))
        .subcommand(ClapCommand::new("doctor").about("检查配置文件合法性及路径有效性"))
        .subcommand(ClapCommand::new("reset").about("恢复项目 100% 全量目录与文件"))
        .allow_external_subcommands(true)
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
    fn rejects_profile_names_that_cannot_be_invoked() {
        for name in ["init", "reapply", "doc", "doctor", "reset", "help"] {
            assert!(validate_profile_name(name).is_err(), "'{name}' 应被拒绝");
        }
        for name in ["", " win-dev", "-x", "my profile"] {
            assert!(validate_profile_name(name).is_err(), "'{name}' 应被拒绝");
        }
        for name in ["win-dev", "rust-only", "android_ui", "iOS"] {
            assert!(validate_profile_name(name).is_ok(), "'{name}' 应被允许");
        }
    }
}
