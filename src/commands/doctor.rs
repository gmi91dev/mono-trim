//! `doctor`：配置语法、字段合法性与物理路径有效性自检。

use std::fs;
use std::path::Path;

use crate::Result;
use crate::cli;
use crate::config::{self, Config};
use crate::sparse::normalize_target;

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

pub fn run(config_path: &Path) -> Result<()> {
    println!("🔍 正在检查 {} ...", config::display_path());
    let config = config::read_config(config_path)?;
    // targets 相对项目根目录解析，支持在子目录中调用
    let root = config::project_root_of(config_path);

    println!(
        "ℹ️ 内置保留命令：{}（profiles 不可同名）。",
        cli::reserved_commands().collect::<Vec<_>>().join(" / ")
    );
    if config.profiles.is_empty() {
        println!("ℹ️ 尚未配置任何 profiles，当前仅可使用内置命令。");
    }

    let mut checker = Checker::default();
    for finding in inspect(&config, root) {
        match finding.level {
            Level::Error => checker.error(finding.message),
            Level::Warn => checker.warn(finding.message),
        }
    }
    checker.finish()
}

/// 逐项检查配置，返回全部结论（纯逻辑，便于单测）
fn inspect(config: &Config, root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();

    for name in config::sorted_profiles(config) {
        let profile = &config.profiles[name];

        // 与内置命令重名 / 含空白等命名会被命令路由抢走，必须提前拦住
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
                "[{name}] 缺少 desc 描述，导出的文档表格会留空。"
            )));
        }
        if profile.targets.is_empty() {
            findings.push(warn(format!(
                "[{name}] targets 为空，执行时不会裁剪任何内容。"
            )));
        }

        for target in &profile.targets {
            // 复用规则生成时的归一化结果，避免两处各自处理前缀
            let normalized = match normalize_target(target) {
                Ok(path) => path,
                Err(message) => {
                    findings.push(error(format!("[{name}] {message}")));
                    continue;
                }
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
            println!("⚠️ 配置文件语法合法，但存在 {} 处警告。", self.warnings);
        } else {
            println!("✅ 配置文件健康检查通过。");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TempDir, write_config};

    fn config_from(body: &str) -> Config {
        toml::from_str(body).unwrap()
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
        assert!(checker.finish().is_ok(), "仅警告不应导致检查失败");

        let mut checker = Checker::default();
        checker.error("错误".to_string());
        let err = checker.finish().unwrap_err();
        assert!(err.contains("1 处错误"), "{err}");
    }

    #[test]
    fn healthy_config_produces_no_findings() {
        let dir = TempDir::new("doctor-ok");
        dir.write("apps/ios/keep.txt", "x");
        let config = config_from(
            r#"[profiles.win-dev]
mode = "black"
desc = "隐藏 iOS 壳"
targets = ["apps/ios/"]
"#,
        );

        assert!(inspect(&config, dir.path()).is_empty());
    }

    #[test]
    fn rejects_profile_names_that_cannot_be_invoked() {
        let dir = TempDir::new("doctor-name");
        let config = config_from(
            r#"[profiles.reset]
mode = "black"
desc = "占用内置命令"
targets = []
"#,
        );

        let findings = inspect(&config, dir.path());
        let errors = messages(&findings, Level::Error);
        assert_eq!(errors.len(), 1, "{findings:?}");
        assert!(errors[0].contains("内置命令重名"), "{errors:?}");
    }

    #[test]
    fn rejects_unknown_mode_and_unsafe_targets() {
        let dir = TempDir::new("doctor-bad");
        let config = config_from(
            r#"[profiles.bad]
mode = "gray"
desc = ""
targets = [".git/config", "../outside"]
"#,
        );

        let findings = inspect(&config, dir.path());
        let errors = messages(&findings, Level::Error);

        assert_eq!(errors.len(), 3, "{findings:?}");
        assert!(errors.iter().any(|m| m.contains("mode")), "{errors:?}");
        assert!(errors.iter().any(|m| m.contains(".git")), "{errors:?}");
        assert!(errors.iter().any(|m| m.contains("..")), "{errors:?}");
        assert!(
            messages(&findings, Level::Warn)
                .iter()
                .any(|m| m.contains("desc")),
            "缺少 desc 应给出警告：{findings:?}"
        );
    }

    #[test]
    fn warns_about_missing_paths_but_stays_ok() {
        let dir = TempDir::new("doctor-missing");
        let config = config_from(
            r#"[profiles.p]
mode = "black"
desc = "拼写错误"
targets = ["nope/"]
"#,
        );

        let findings = inspect(&config, dir.path());
        assert!(messages(&findings, Level::Error).is_empty());
        assert!(
            messages(&findings, Level::Warn)
                .iter()
                .any(|m| m.contains("不存在")),
            "{findings:?}"
        );
    }

    #[test]
    fn warns_when_directory_lacks_trailing_slash() {
        let dir = TempDir::new("doctor-slash");
        dir.write("apps/ios/keep.txt", "x");
        let config = config_from(
            r#"[profiles.p]
mode = "black"
desc = "目录少了斜杠"
targets = ["apps/ios"]
"#,
        );

        let findings = inspect(&config, dir.path());
        assert!(
            messages(&findings, Level::Warn)
                .iter()
                .any(|m| m.contains("'/' 结尾")),
            "{findings:?}"
        );
    }

    #[test]
    fn run_reports_summary_and_status() {
        let dir = TempDir::new("doctor-run");
        let ok_path = write_config(
            &dir,
            r#"[profiles.win-dev]
mode = "black"
desc = "正常"
targets = []
"#,
        );
        assert!(run(&ok_path).is_ok(), "仅有警告时应通过");

        let bad_path = write_config(
            &dir,
            r#"[profiles.reset]
mode = "gray"
desc = "非法"
targets = []
"#,
        );
        let err = run(&bad_path).unwrap_err();
        assert!(err.contains("2 处错误"), "{err}");
    }
}
