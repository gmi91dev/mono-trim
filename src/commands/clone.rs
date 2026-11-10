//! `clone`：克隆仓库时只检出 mono-trim 的配置文件。
//!
//! 其余文件等执行 `mtrim <profile>` 时再按稀疏规则拉取，避免克隆整仓。

use std::path::PathBuf;

use crate::Result;
use crate::commands::{apply, ls};
use crate::config::Project;
use crate::git::{self, SystemGit};
use crate::util::write_file;

/// 克隆时只放行的路径（相对仓库根）
const CONFIG_PATTERNS: [&str; 2] = ["/.mono-trim/config.toml", "/mono-trim.toml"];

pub fn run(repository: &str, directory: Option<&str>, profile: Option<&str>) -> Result<()> {
    let dir = match directory {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(default_dir_name(repository)),
    };

    if dir.exists() {
        let non_empty = std::fs::read_dir(&dir)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
        if non_empty {
            return Err(format!(
                "目录 {} 已存在且不为空，请换一个目录名",
                dir.display()
            ));
        }
    }

    println!("⏳ 正在克隆 {} -> {} ...", repository, dir.display());
    let dir_arg = dir.to_string_lossy().into_owned();
    let partial = git::git_ok(
        &SystemGit,
        &[
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            repository,
            &dir_arg,
        ],
    );
    if let Err(error) = partial {
        // 只有服务端不支持 partial clone 时才退回普通克隆，网络等错误直接上报
        if !should_retry_without_filter(&error) {
            return Err(error);
        }
        println!("⚠️  服务端不支持部分克隆，改为普通克隆（历史会完整下载）");
        git::git_ok(
            &SystemGit,
            &["clone", "--no-checkout", repository, &dir_arg],
        )?;
    }

    // 非 cone 模式：只放行两份配置文件
    git::git_ok_in(&SystemGit, &dir, &["sparse-checkout", "init", "--no-cone"])?;
    let sparse_path = git::git_path_in(&SystemGit, &dir, "info/sparse-checkout")?;
    write_file(&sparse_path, &format!("{}\n", CONFIG_PATTERNS.join("\n")))?;
    git::git_ok_in(&SystemGit, &dir, &["checkout"])?;

    let root = std::env::current_dir()
        .map_err(|e| format!("无法获取当前目录：{e}"))?
        .join(&dir);
    let project = Project::from_root(&root);

    if !project.is_initialized() {
        println!(
            "⚠️  该仓库未使用 mono-trim（未找到 {}）",
            project.system_config_display()
        );
        println!(
            "ℹ️ 如需完整检出：git -C {} sparse-checkout disable",
            dir.display()
        );
        return Ok(());
    }

    println!("✨ 已克隆到 {}（仅配置文件）", dir.display());
    println!("ℹ️ 可用命令：");
    ls::run(&project, false)?;

    match profile {
        Some(name) => {
            std::env::set_current_dir(&root)
                .map_err(|e| format!("切换到 {} 失败：{e}", root.display()))?;
            println!();
            apply::profile(&project, name, &SystemGit)?;
        }
        None => {
            println!();
            println!("👉 进入目录后执行：cd {} && mtrim <命令>", dir.display());
        }
    }

    Ok(())
}

/// 判断 clone 失败是否因为服务端不支持 partial clone
fn should_retry_without_filter(error: &str) -> bool {
    let lowered = error.to_lowercase();
    ["filter", "partial clone", "not supported", "not recognized"]
        .iter()
        .any(|needle| lowered.contains(needle))
}

/// 从仓库地址推导默认目录名
pub fn default_dir_name(repository: &str) -> String {
    let trimmed = repository
        .trim_end_matches(['/', '\\'])
        .trim_end_matches(".git");
    let name = trimmed.rsplit(['/', '\\', ':']).next().unwrap_or(trimmed);
    if name.is_empty() {
        "repo".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_directory_name_from_common_urls() {
        assert_eq!(
            default_dir_name("https://github.com/gmi91dev/mono-trim.git"),
            "mono-trim"
        );
        assert_eq!(
            default_dir_name("git@github.com:gmi91dev/mono-trim.git"),
            "mono-trim"
        );
        assert_eq!(default_dir_name("https://example.com/team/repo/"), "repo");
        assert_eq!(default_dir_name("repo"), "repo");
        assert_eq!(default_dir_name(r"C:\work\repo"), "repo");
        assert_eq!(default_dir_name(""), "repo");
    }

    #[test]
    fn retries_without_filter_only_for_filter_errors() {
        assert!(should_retry_without_filter(
            "warning: filtering not recognized by server, ignoring"
        ));
        assert!(should_retry_without_filter(
            "error: server does not support filter"
        ));
        assert!(!should_retry_without_filter(
            "fatal: unable to access: Could not connect to server"
        ));
        assert!(!should_retry_without_filter("fatal: repository not found"));
    }

    #[test]
    fn config_patterns_are_repo_relative() {
        for pattern in CONFIG_PATTERNS {
            assert!(pattern.starts_with('/'), "{pattern}");
        }
    }
}
