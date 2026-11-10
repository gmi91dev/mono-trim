//! `doctor`：只校验系统配置与用户配置的合法性。

use std::fs;
use std::path::Path;

use crate::Result;
use crate::cli;
use crate::config::{self, Config, Project, SystemConfig, UserConfig};
use crate::sparse::normalize_target;
use crate::version;

/// 检查项的严重级别
#[derive(Debug, PartialEq, Eq)]
enum Level {
    Error,
    Warn,
}

/// 单条检查结论
#[derive(Debug)]
struct Finding {
    level: Level,
    message: String,
}

fn error(message: String) -> Finding {
    Finding {
        level: Level::Error,
        message,
    }
}

fn warn(message: String) -> Finding {
    Finding {
        level: Level::Warn,
        message,
    }
}

pub fn run(project: &Project) -> Result<()> {
    let has_system = project.is_initialized();
    let has_user = project.user_config().exists();
    if !has_system && !has_user {
        return Err(format!(
            "未发现配置文件，请先运行 mtrim init（预期位置：{}、{}）",
            project.system_config_display(),
            project.user_config_display()
        ));
    }

    println!(
        "🔍 正在检查 {} 与 {} ...",
        project.system_config_display(),
        project.user_config_display()
    );

    let mut findings = Vec::new();
    let mut system = None;
    let mut user = None;

    if has_system {
        match config::read_system(&project.system_config()) {
            Ok(parsed) => {
                findings.extend(inspect_system(&parsed));
                system = Some(parsed);
            }
            Err(e) => findings.push(error(e)),
        }
    } else {
        findings.push(warn(format!(
            "缺少系统配置 {}，可运行 mtrim init 生成",
            project.system_config_display()
        )));
    }

    if has_user {
        match config::read_user(&project.user_config()) {
            Ok(parsed) => {
                findings.extend(inspect_user(&parsed));
                user = Some(parsed);
            }
            Err(e) => findings.push(error(e)),
        }
    }

    if let (Some(system), Some(user)) = (&system, &user) {
        findings.extend(inspect_profiles(&merge(system, user), project.root()));
    }

    let mut checker = Checker::default();
    for finding in findings {
        match finding.level {
            Level::Error => checker.error(finding.message),
            Level::Warn => checker.warn(finding.message),
        }
    }
    checker.finish()
}

fn merge(system: &SystemConfig, user: &UserConfig) -> Config {
    let mut profiles = system.profiles.clone();
    profiles.extend(user.profiles.clone());
    Config {
        version: system.version.clone().unwrap_or_default(),
        doc_target: user
            .doc_target
            .clone()
            .or_else(|| system.doc_target.clone())
            .unwrap_or_else(|| config::DEFAULT_DOC_TARGET.to_string()),
        profiles,
    }
}

fn inspect_system(system: &SystemConfig) -> Vec<Finding> {
    let mut findings = Vec::new();

    match system.version.as_deref() {
        Some(value) if version::is_valid(value) => {}
        Some(value) => findings.push(error(format!(
            "系统配置 version = \"{value}\" 不是合法的 x.y.z 版本号。"
        ))),
        None => findings.push(error("系统配置缺少 version 字段。".into())),
    }

    if let Some(doc_target) = system.doc_target.as_deref() {
        findings.extend(inspect_doc_target(doc_target, "系统配置"));
    }

    findings.extend(inspect_profile_entries(&system.profiles));
    findings
}

fn inspect_user(user: &UserConfig) -> Vec<Finding> {
    let mut findings = Vec::new();

    if let Some(doc_target) = user.doc_target.as_deref() {
        findings.extend(inspect_doc_target(doc_target, "用户配置"));
    }
    findings.extend(inspect_profile_entries(&user.profiles));
    findings
}

fn inspect_doc_target(doc_target: &str, label: &str) -> Vec<Finding> {
    if doc_target.trim().is_empty() {
        return vec![error(format!("{label} doc_target 为空。"))];
    }
    // Windows 上 "/abs" 不算 is_absolute，这里统一按"以分隔符开头"处理
    let absolute_like = Path::new(doc_target).is_absolute()
        || doc_target.starts_with('/')
        || doc_target.starts_with('\\');
    if absolute_like {
        return vec![error(format!(
            "{label} doc_target '{doc_target}' 必须是相对项目根目录的路径。"
        ))];
    }
    Vec::new()
}

fn inspect_profile_entries(
    profiles: &std::collections::HashMap<String, config::Profile>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (name, profile) in profiles {
        if let Err(message) = cli::validate_profile_name(name) {
            findings.push(error(message));
        }
        if !matches!(profile.mode.as_str(), "black" | "white") {
            findings.push(error(format!(
                "[{name}] mode = \"{}\" 非法，仅支持 \"black\" / \"white\"。",
                profile.mode
            )));
        }
        if profile.desc.trim().is_empty() {
            findings.push(warn(format!(
                "[{name}] 缺少 desc 描述，mtrim ls / doc 会留空。"
            )));
        }
        if profile.targets.is_empty() {
            findings.push(warn(format!(
                "[{name}] targets 为空，执行时不会裁剪任何内容。"
            )));
        }
        for target in &profile.targets {
            if let Err(message) = normalize_target(target) {
                findings.push(error(format!("[{name}] {message}")));
            }
        }
    }

    findings
}

/// 合并后检查路径是否真实存在、目录斜杠规则是否正确
fn inspect_profiles(config: &Config, root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();

    for name in config.sorted_profile_names() {
        let profile = &config.profiles[name];
        for target in &profile.targets {
            let Ok(normalized) = normalize_target(target) else {
                continue;
            };

            match fs::symlink_metadata(root.join(normalized.trim_start_matches('/'))) {
                Err(_) => findings.push(warn(format!(
                    "[{name}] 路径 '{target}' 在当前物理硬盘上不存在（拼写错误？）。"
                ))),
                Ok(metadata) => {
                    let trailing_slash = target.trim_end().ends_with('/');
                    if metadata.is_dir() && !trailing_slash {
                        findings.push(warn(format!("[{name}] 目录 '{target}' 建议以 '/' 结尾。")));
                    } else if !metadata.is_dir() && trailing_slash {
                        findings.push(warn(format!(
                            "[{name}] 单文件 '{target}' 不应以 '/' 结尾。"
                        )));
                    }
                }
            }
        }
    }

    findings
}

/// 汇总检查结果：错误数决定退出码，警告只提示
#[derive(Default)]
struct Checker {
    errors: usize,
    warnings: usize,
}

impl Checker {
    fn error(&mut self, message: String) {
        self.errors += 1;
        println!("❌ {message}");
    }

    fn warn(&mut self, message: String) {
        self.warnings += 1;
        println!("⚠️  {message}");
    }

    fn finish(self) -> Result<()> {
        if self.errors > 0 {
            return Err(format!(
                "检查未通过：{} 处错误，{} 处警告。",
                self.errors, self.warnings
            ));
        }
        if self.warnings > 0 {
            println!("⚠️ 配置合法，但存在 {} 处警告。", self.warnings);
        } else {
            println!("✅ 配置检查通过。");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn project(dir: &TempDir) -> Project {
        Project::from_root(dir.path())
    }

    fn messages(findings: &[Finding], level: Level) -> Vec<&str> {
        findings
            .iter()
            .filter(|finding| finding.level == level)
            .map(|finding| finding.message.as_str())
            .collect()
    }

    #[test]
    fn checker_treats_warnings_as_ok_and_errors_as_failure() {
        let mut checker = Checker::default();
        checker.warn("警告".to_string());
        assert!(checker.finish().is_ok());

        let mut checker = Checker::default();
        checker.error("错误".to_string());
        assert!(checker.finish().unwrap_err().contains("1 处错误"));
    }

    #[test]
    fn healthy_configs_produce_no_findings() {
        let dir = TempDir::new("doctor-ok");
        dir.write("apps/ios/keep.txt", "x");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write(
            "mono-trim.toml",
            r#"[profiles.win-dev]
mode = "black"
desc = "隐藏 iOS 壳"
targets = ["apps/ios/"]
"#,
        );

        run(&project(&dir)).unwrap();
    }

    #[test]
    fn reports_invalid_version() {
        let dir = TempDir::new("doctor-version");
        dir.write(".mono-trim/config.toml", "version = \"abc\"\n");

        let err = run(&project(&dir)).unwrap_err();

        assert!(err.contains("1 处错误"), "{err}");
    }

    #[test]
    fn reports_reserved_profile_name() {
        let dir = TempDir::new("doctor-reserved");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write(
            "mono-trim.toml",
            "[profiles.init]\nmode = \"black\"\ndesc = \"x\"\n",
        );

        let err = run(&project(&dir)).unwrap_err();

        assert!(err.contains("处错误"), "{err}");
    }

    #[test]
    fn reports_syntax_error_and_unsafe_targets() {
        let dir = TempDir::new("doctor-bad");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write(
            "mono-trim.toml",
            "[profiles.bad]\nmode = \"gray\"\ndesc = \"\"\ntargets = [\".git/config\", \"../outside\"]\n",
        );

        let err = run(&project(&dir)).unwrap_err();

        assert!(err.contains("3 处错误"), "{err}");
    }

    #[test]
    fn reports_toml_syntax_error_with_file_name() {
        let dir = TempDir::new("doctor-syntax");
        dir.write(".mono-trim/config.toml", "version = ");
        dir.write(
            "mono-trim.toml",
            "[profiles.ok]\nmode = \"black\"\ndesc = \"x\"\n",
        );

        let err = run(&project(&dir)).unwrap_err();

        assert!(err.contains("检查未通过"), "{err}");
    }

    #[test]
    fn warns_when_target_missing() {
        let dir = TempDir::new("doctor-missing");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write(
            "mono-trim.toml",
            "[profiles.p]\nmode = \"black\"\ndesc = \"拼写错误\"\ntargets = [\"nope/\"]\n",
        );

        run(&project(&dir)).expect("仅有警告时应通过");
    }

    #[test]
    fn fails_when_no_config_files_exist() {
        let dir = TempDir::new("doctor-none");
        assert!(run(&project(&dir)).is_err());
    }

    #[test]
    fn inspect_helpers_are_pure() {
        let system = SystemConfig {
            version: Some("0.1.0".into()),
            doc_target: Some("README.md".into()),
            profiles: Default::default(),
        };
        assert!(inspect_system(&system).is_empty());

        let user = UserConfig {
            doc_target: Some("/abs/path.md".into()),
            profiles: Default::default(),
        };
        assert_eq!(messages(&inspect_user(&user), Level::Error).len(), 1);
    }
}
