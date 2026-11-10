//! 项目配置：系统配置（`.mono-trim/config.toml`）+ 用户配置（`mono-trim.toml`）。
//!
//! - 系统配置由工具生成（`init` / `update` 会整体重写），记录版本号与默认值；
//! - 用户配置由用户维护，只写需要覆盖的字段与自定义 `profiles`；
//! - 运行时按「系统 ← 用户」合并，用户字段优先。

use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::git::{GitRunner, project_root};
use crate::util::write_file;
use crate::version;

/// 工具封闭目录（系统配置所在）
pub const TOOL_DIR: &str = ".mono-trim";
/// 系统配置文件名
pub const SYSTEM_CONFIG: &str = "config.toml";
/// 用户配置文件名（项目根目录）
pub const USER_CONFIG: &str = "mono-trim.toml";
/// 默认文档输出位置（相对项目根目录）
pub const DEFAULT_DOC_TARGET: &str = "README.md";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 系统配置：工具维护，`init` / `update` 会用内置默认值整体重写
#[derive(Deserialize, Debug, Default)]
pub struct SystemConfig {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub doc_target: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

/// 用户配置：用户维护，只写覆盖项与自定义命令
#[derive(Deserialize, Debug, Default)]
pub struct UserConfig {
    #[serde(default)]
    pub doc_target: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Profile {
    pub mode: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub targets: Vec<String>,
}

/// 合并后的最终配置
#[derive(Debug)]
pub struct Config {
    pub version: String,
    pub doc_target: String,
    pub profiles: HashMap<String, Profile>,
}

impl Config {
    /// profile 名称按字典序排列，保证输出稳定
    pub fn sorted_profile_names(&self) -> Vec<&String> {
        let mut names: Vec<&String> = self.profiles.keys().collect();
        names.sort();
        names
    }

    /// 当前 CLI 与项目记录版本不一致时给出提示
    pub fn version_hint(&self) -> Option<String> {
        let current = current_version();
        if self.version == current {
            return None;
        }
        if version::is_newer(&self.version, current) {
            Some(format!(
                "项目配置由更高版本（{}）写入，当前 mtrim 为 {current}，建议升级系统 CLI",
                self.version
            ))
        } else {
            Some(format!(
                "项目配置版本（{}）落后于当前 mtrim（{current}），可运行 mtrim update 同步",
                self.version
            ))
        }
    }
}

/// 项目上下文：根目录与两份配置的位置
pub struct Project {
    root: PathBuf,
}

impl Project {
    pub fn detect(git: &dyn GitRunner) -> Self {
        Self {
            root: project_root(git),
        }
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tool_dir(&self) -> PathBuf {
        self.root.join(TOOL_DIR)
    }

    pub fn system_config(&self) -> PathBuf {
        self.tool_dir().join(SYSTEM_CONFIG)
    }

    pub fn user_config(&self) -> PathBuf {
        self.root.join(USER_CONFIG)
    }

    pub fn system_config_display(&self) -> String {
        format!("./{TOOL_DIR}/{SYSTEM_CONFIG}")
    }

    pub fn user_config_display(&self) -> String {
        format!("./{USER_CONFIG}")
    }

    pub fn is_initialized(&self) -> bool {
        self.system_config().exists()
    }

    /// 读取并合并两份配置（缺失的文件按默认值处理）
    pub fn load(&self) -> Result<Config> {
        let system: SystemConfig = read_optional(&self.system_config())?.unwrap_or_default();
        let user: UserConfig = read_optional(&self.user_config())?.unwrap_or_default();

        let mut profiles = system.profiles;
        profiles.extend(user.profiles);

        Ok(Config {
            version: system
                .version
                .unwrap_or_else(|| current_version().to_string()),
            doc_target: user
                .doc_target
                .or(system.doc_target)
                .unwrap_or_else(|| DEFAULT_DOC_TARGET.to_string()),
            profiles,
        })
    }

    /// 只读系统配置，主要用于 `update` / `doctor`
    pub fn load_system(&self) -> Result<SystemConfig> {
        Ok(read_optional(&self.system_config())?.unwrap_or_default())
    }

    /// 用内置默认值重写系统配置（不触碰用户配置）
    pub fn write_system_config(&self, version: &str) -> Result<()> {
        fs::create_dir_all(self.tool_dir())
            .map_err(|e| format!("创建 {TOOL_DIR} 目录失败：{e}"))?;
        write_file(&self.system_config(), &system_config_content(version))
    }

    /// 用户配置不存在时写入最小模板，返回是否新建
    pub fn ensure_user_config(&self) -> Result<bool> {
        if self.user_config().exists() {
            return Ok(false);
        }
        write_file(&self.user_config(), USER_CONFIG_TEMPLATE)?;
        Ok(true)
    }

    /// 文档输出位置（相对项目根目录解析）
    pub fn doc_target_path(&self, config: &Config) -> PathBuf {
        self.root.join(&config.doc_target)
    }
}

/// 系统配置内容：内置默认值 + 版本号
pub fn system_config_content(version: &str) -> String {
    format!(
        "# 系统配置：由 mono-trim 生成，请勿手工修改（mtrim init / update 会整体重写本文件）\n\
         version = \"{version}\"\n\
         doc_target = \"{DEFAULT_DOC_TARGET}\"\n"
    )
}

pub const USER_CONFIG_TEMPLATE: &str = r#"# 用户配置：只写需要覆盖或新增的内容
#
# 自定义裁剪命令：
#   [profiles.命令名]
#   mode = "black"   # black：保留全部，排除 targets；white：仅保留 targets
#   desc = "命令说明（mtrim ls / doc 会展示）"
#   targets = ["apps/ios/", "assets/large_video.mp4"]
#
# 覆盖系统默认的文档输出位置（相对项目根目录）：
# doc_target = "docs/commands.md"
"#;

pub fn read_system(path: &Path) -> Result<SystemConfig> {
    read_required(path, "系统配置")
}

pub fn read_user(path: &Path) -> Result<UserConfig> {
    read_required(path, "用户配置")
}

fn read_required<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("无法读取{label} {}（{e}）", display(path)))?;
    toml::from_str(&content).map_err(|e| format!("{label} {} 语法错误：{e}", display(path)))
}

fn read_optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content)
            .map(Some)
            .map_err(|e| format!("{} 语法错误：{e}", display(path))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("无法读取 {}（{e}）", display(path))),
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn write(dir: &TempDir, relative: &str, content: &str) -> PathBuf {
        dir.write(relative, content)
    }

    #[test]
    fn user_config_overrides_system_config() {
        let dir = TempDir::new("config-merge");
        write(
            &dir,
            ".mono-trim/config.toml",
            "version = \"0.1.0\"\ndoc_target = \"README.md\"\n",
        );
        write(
            &dir,
            "mono-trim.toml",
            "doc_target = \"docs/commands.md\"\n\n[profiles.win-dev]\nmode = \"black\"\ndesc = \"x\"\ntargets = []\n",
        );
        let project = Project::from_root(dir.path());

        let config = project.load().unwrap();

        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.doc_target, "docs/commands.md");
        assert!(config.profiles.contains_key("win-dev"));
    }

    #[test]
    fn falls_back_to_builtin_defaults() {
        let dir = TempDir::new("config-empty");
        let project = Project::from_root(dir.path());

        let config = project.load().unwrap();

        assert_eq!(config.version, current_version());
        assert_eq!(config.doc_target, DEFAULT_DOC_TARGET);
        assert!(config.profiles.is_empty());
        assert!(!project.is_initialized());
    }

    #[test]
    fn user_profiles_override_same_name_system_profile() {
        let dir = TempDir::new("config-profile-merge");
        write(
            &dir,
            ".mono-trim/config.toml",
            "version = \"0.1.0\"\n\n[profiles.dev]\nmode = \"black\"\ndesc = \"系统\"\ntargets = [\"a/\"]\n",
        );
        write(
            &dir,
            "mono-trim.toml",
            "[profiles.dev]\nmode = \"white\"\ndesc = \"用户\"\ntargets = [\"b/\"]\n",
        );

        let config = Project::from_root(dir.path()).load().unwrap();

        assert_eq!(config.profiles["dev"].mode, "white");
        assert_eq!(config.profiles["dev"].desc, "用户");
    }

    #[test]
    fn reports_syntax_errors_with_file_name() {
        let dir = TempDir::new("config-broken");
        write(&dir, "mono-trim.toml", "mode = ");

        let err = Project::from_root(dir.path()).load().unwrap_err();

        assert!(err.contains("mono-trim.toml"), "{err}");
        assert!(err.contains("语法错误"), "{err}");
    }

    #[test]
    fn system_config_is_rewritten_but_user_config_is_kept() {
        let dir = TempDir::new("config-write");
        write(
            &dir,
            "mono-trim.toml",
            "[profiles.keep]\nmode = \"black\"\ntargets = []\n",
        );
        let project = Project::from_root(dir.path());

        project.write_system_config("9.9.9").unwrap();
        project.ensure_user_config().unwrap();

        assert_eq!(
            project.load_system().unwrap().version.as_deref(),
            Some("9.9.9")
        );
        assert!(dir.read("mono-trim.toml").contains("profiles.keep"));
    }

    #[test]
    fn version_hint_points_to_upgrade_or_update() {
        let dir = TempDir::new("config-hint");
        write(&dir, ".mono-trim/config.toml", "version = \"99.0.0\"\n");
        let newer = Project::from_root(dir.path()).load().unwrap();
        assert!(newer.version_hint().unwrap().contains("建议升级系统 CLI"));

        write(&dir, ".mono-trim/config.toml", "version = \"0.0.0\"\n");
        let older = Project::from_root(dir.path()).load().unwrap();
        assert!(older.version_hint().unwrap().contains("mtrim update"));
    }
}
