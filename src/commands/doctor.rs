//! `doctor`：配置语法、字段合法性与物理路径有效性自检。

use std::fs;
use std::path::Path;

use crate::Result;
use crate::cli;
use crate::config::{self, CONFIG_NAME, TOOL_DIR};
use crate::sparse::normalize_target;

pub fn run(config_path: &Path) -> Result<()> {
    println!("🔍 正在检查 ./{TOOL_DIR}/{CONFIG_NAME} ...");
    let config = config::read_config(config_path)?;
    // targets 相对项目根目录解析，支持在子目录中调用
    let root = config_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."));

    let mut checker = Checker::default();
    println!(
        "ℹ️ 内置保留命令：{}（profiles 不可同名）。",
        cli::reserved_commands().collect::<Vec<_>>().join(" / ")
    );
    let names = config::sorted_profiles(&config);
    if names.is_empty() {
        println!("ℹ️ 尚未配置任何 profiles，当前仅可使用内置命令。");
    }

    for name in names {
        let profile = &config.profiles[name];

        // 与内置命令重名 / 含空白等命名会被命令路由抢走，必须提前拦住
        if let Err(message) = cli::validate_profile_name(name) {
            checker.error(message);
        }
        if !matches!(profile.mode.as_str(), "black" | "white") {
            checker.error(format!(
                "[{name}] mode = \"{}\" 非法，仅支持 \"black\" / \"white\"。",
                profile.mode
            ));
        }
        if profile.desc.trim().is_empty() {
            checker.warn(format!("[{name}] 缺少 desc 描述，导出的文档表格会留空。"));
        }
        if profile.targets.is_empty() {
            checker.warn(format!("[{name}] targets 为空，执行时不会裁剪任何内容。"));
        }

        for target in &profile.targets {
            if let Err(message) = normalize_target(target) {
                checker.error(format!("[{name}] {message}"));
                continue;
            }

            match fs::symlink_metadata(root.join(target.trim_start_matches('/'))) {
                Err(_) => checker.warn(format!(
                    "[{name}] 路径 '{target}' 在当前物理硬盘上不存在（拼写错误？）。"
                )),
                Ok(metadata) => {
                    let trailing_slash = target.trim_end().ends_with('/');
                    if metadata.is_dir() && !trailing_slash {
                        checker.warn(format!("[{name}] 目录 '{target}' 建议以 '/' 结尾。"));
                    } else if !metadata.is_dir() && trailing_slash {
                        checker.warn(format!("[{name}] 单文件 '{target}' 不应以 '/' 结尾。"));
                    }
                }
            }
        }
    }

    checker.finish()
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
