//! `clone`：标准克隆仓库，并识别其中的 mono-trim 配置（可选地立即执行某条命令）。

use std::path::PathBuf;

use crate::Result;
use crate::commands::{apply, ls};
use crate::config::Project;
use crate::git::{GitRunner, SystemGit};

pub fn run(repository: &str, directory: Option<&str>, command: Option<&str>) -> Result<()> {
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

    println!("⏳ 正在克隆 {} -> {}", repository, dir.display());
    println!("   （大型仓库可能需要几分钟，git 会显示下载进度）");
    let started = std::time::Instant::now();
    let dir_arg = dir.to_string_lossy().into_owned();
    // 继承终端输出，让 git 的进度条/统计直接展示出来
    SystemGit.run_inherit(&["clone", repository, &dir_arg])?;
    println!("✅ 克隆完成，用时 {:.1}s", started.elapsed().as_secs_f64());

    let root = std::env::current_dir()
        .map_err(|e| format!("无法获取当前目录：{e}"))?
        .join(&dir);
    let project = Project::from_root(&root);

    if !project.is_initialized() {
        println!("✨ 已克隆到 {}", dir.display());
        println!("⚠️  该仓库尚未使用 mono-trim 管理（未找到配置文件）");
        println!(
            "ℹ️ 如需使用，请进入该目录执行：cd {} && mtrim init",
            dir.display()
        );
        if command.is_some() {
            return Err("该仓库尚未使用 mono-trim 管理，无法执行命令".into());
        }
        return Ok(());
    }

    println!("✨ 已克隆到 {}（未做裁剪，工作空间为全量）", dir.display());
    println!("ℹ️ 可用命令：");
    ls::run(&project, false)?;

    match command {
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
        assert_eq!(default_dir_name(""), "repo");
        assert_eq!(default_dir_name(r"C:\work\repo"), "repo");
    }
}
