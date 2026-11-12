//! `init`：用内置默认值重新初始化项目配置（永不改动用户配置）。

use crate::Result;
use crate::config::{Project, USER_CONFIG, current_version};
use crate::git::SystemGit;

pub fn run(project: &Project) -> Result<()> {
    let created_user = project.ensure_user_config()?;
    project.write_system_config(current_version())?;

    println!("✅ 已初始化项目配置（version = {}）", current_version());
    println!(
        "ℹ️ 系统配置 {}（由工具维护，init / update 会重写）",
        project.system_config_display()
    );
    if created_user {
        println!(
            "ℹ️ 用户配置 {}（请在其中编写 profiles）",
            project.user_config_display()
        );
    } else {
        println!(
            "ℹ️ 用户配置 {} 已存在，保持不动",
            project.user_config_display()
        );
    }
    let ignored = project.ignored_configs(&SystemGit)?;
    if !ignored.is_empty() {
        for (display, rule) in ignored {
            println!("⚠️  {display} 被 .gitignore 忽略（{rule}），配置文件需要提交给团队");
        }
        println!(
            "   修复：在 .gitignore 中添加例外  !.mono-trim/  与  !{USER_CONFIG}（或 git add -f）"
        );
    }

    println!("👉 编辑完成后运行 mtrim doctor 校验");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn creates_both_config_files() {
        let dir = TempDir::new("init-fresh");
        let project = Project::from_root(dir.path());

        run(&project).unwrap();

        assert!(dir.exists(".mono-trim/config.toml"));
        assert!(dir.exists("mono-trim.toml"));
        assert_eq!(
            project.load_system().unwrap().version.as_deref(),
            Some(current_version())
        );
    }

    #[test]
    fn rewrites_system_config_but_keeps_user_config() {
        let dir = TempDir::new("init-rerun");
        dir.write(
            "mono-trim.toml",
            "[profiles.keep]\nmode = \"black\"\ntargets = []\n",
        );
        dir.write(".mono-trim/config.toml", "version = \"0.0.1\"\n");
        let project = Project::from_root(dir.path());

        run(&project).unwrap();

        assert_eq!(
            project.load_system().unwrap().version.as_deref(),
            Some(current_version())
        );
        assert!(dir.read("mono-trim.toml").contains("profiles.keep"));
    }
}
