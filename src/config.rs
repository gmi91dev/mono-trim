//! `config.toml` 的数据结构、模板与读取。

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::cli::COMMAND;
use crate::git::{GitRunner, project_root};

/// 工具自身与配置所在的封闭目录（严禁被裁剪）
pub const TOOL_DIR: &str = ".mono-trim";
pub const CONFIG_NAME: &str = "config.toml";
pub const DEFAULT_DOC_TARGET: &str = "../README.md";

const TEMPLATE: &str = r#"# =====================================================================
#                 mono-trim 工作空间裁剪引擎配置文件
# =====================================================================

# ✅ 重要：每次修改本文件后，请务必先运行一次 `{command} doctor` 校验配置，
#    确认无❌错误后再执行自定义裁剪命令，避免非法配置破坏工作空间。

doc_target = "{doc_target}"

# ⛔ 系统保留命令（内置命令，禁止用作 profiles 名称，否则永远无法触发）：
#   {reserved}

# 🛠️ 语法结构模板：
#   [profiles.你的自定义子命令名称]
#   mode = "black" 或 "white"
#   desc = "关于这个自定义命令的作用描述"
#   targets = [ "apps/ios/", "assets/large_video.mp4" ]

# ⚠️ 保护规则：`/`（项目根）、`.git/`、`.mono-trim/` 永远不可被裁剪。
"#;

/// 初始化时写入的完整配置模板（保留命令列表由 `cli` 提供，避免两处漂移）
pub fn template() -> String {
    let reserved = crate::cli::reserved_commands()
        .collect::<Vec<_>>()
        .join(" / ");
    TEMPLATE
        .replace("{reserved}", &reserved)
        .replace("{command}", COMMAND)
        .replace("{doc_target}", DEFAULT_DOC_TARGET)
}

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    #[serde(default)]
    pub doc_target: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Deserialize, Debug)]
pub struct Profile {
    pub mode: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub targets: Vec<String>,
}

/// 配置文件路径：以 Git 顶层目录为基准，支持在子目录中调用
pub fn path(git: &dyn GitRunner) -> PathBuf {
    project_root(git).join(TOOL_DIR).join(CONFIG_NAME)
}

/// 面向用户的配置文件相对路径（提示语统一使用）
pub fn display_path() -> String {
    format!("./{TOOL_DIR}/{CONFIG_NAME}")
}

/// 由配置文件路径反推 `.mono-trim` 目录
pub fn tool_dir_of(config_path: &Path) -> &Path {
    config_path.parent().unwrap_or(Path::new("."))
}

/// 由配置文件路径反推项目根目录
pub fn project_root_of(config_path: &Path) -> &Path {
    tool_dir_of(config_path).parent().unwrap_or(Path::new("."))
}

pub fn read_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path).map_err(|e| {
        format!(
            "未发现或无法读取配置文件 {}（{e}），请先运行 {COMMAND} init。",
            display_path()
        )
    })?;
    toml::from_str(&content).map_err(|e| format!("配置文件 {} 语法错误：{e}", display_path()))
}

/// 配置中的 profile 名称按字典序排列，保证输出稳定
pub fn sorted_profiles(config: &Config) -> Vec<&String> {
    let mut names: Vec<&String> = config.profiles.keys().collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn template_is_valid_config_and_mentions_reserved_commands() {
        let text = template();

        assert!(!text.contains("{command}"), "命令名占位符应被替换");
        assert!(!text.contains("{reserved}"), "保留命令占位符应被替换");
        assert!(text.contains("mtrim doctor"), "模板应提示运行 doctor");
        assert!(text.contains("init / reapply / doc / doctor / reset / help"));

        // 生成的模板必须能被自身解析，否则 init 完就会 doctor 失败
        let config: Config = toml::from_str(&text).unwrap();
        assert!(config.profiles.is_empty());
        assert_eq!(config.doc_target.as_deref(), Some(DEFAULT_DOC_TARGET));
    }

    #[test]
    fn read_config_reports_missing_file() {
        let dir = TempDir::new("config-missing");
        let err = read_config(&dir.path().join(CONFIG_NAME)).unwrap_err();
        assert!(err.contains("mtrim init"), "应提示先执行 init：{err}");
    }

    #[test]
    fn read_config_reports_syntax_error() {
        let dir = TempDir::new("config-syntax");
        let path = dir.write(CONFIG_NAME, r#"mode = "#);
        let err = read_config(&path).unwrap_err();
        assert!(err.contains("语法错误"), "应报告 TOML 语法错误：{err}");
    }

    #[test]
    fn read_config_applies_defaults() {
        let dir = TempDir::new("config-defaults");
        let path = dir.write(
            CONFIG_NAME,
            r#"[profiles.only-mode]
mode = "black"
"#,
        );
        let config = read_config(&path).unwrap();
        let profile = config.profiles.get("only-mode").unwrap();

        assert_eq!(profile.mode, "black");
        assert!(profile.desc.is_empty(), "desc 应默认为空");
        assert!(profile.targets.is_empty(), "targets 应默认为空");
        assert!(config.doc_target.is_none(), "doc_target 应可缺省");
    }

    #[test]
    fn sorted_profiles_orders_by_name() {
        let dir = TempDir::new("config-sorted");
        let path = dir.write(
            CONFIG_NAME,
            r#"[profiles.zeta]
mode = "black"

[profiles.alpha]
mode = "white"

[profiles.mid]
mode = "black"
"#,
        );
        let config = read_config(&path).unwrap();
        let names: Vec<&str> = sorted_profiles(&config)
            .into_iter()
            .map(|name| name.as_str())
            .collect();

        assert_eq!(names, ["alpha", "mid", "zeta"]);
    }
}
