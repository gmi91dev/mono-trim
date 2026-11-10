//! `ls`：列出可用的裁剪命令（`-a` 同时列出内置命令）。

use crate::Result;
use crate::cli;
use crate::config::Project;

pub fn run(project: &Project, all: bool) -> Result<()> {
    let config = project.load()?;

    let mut rows: Vec<(String, String, String)> = Vec::new();
    if all {
        for (name, desc) in cli::BUILTIN_DESCRIPTIONS {
            rows.push((name.to_string(), "内置".to_string(), desc.to_string()));
        }
    }
    for name in config.sorted_profile_names() {
        let profile = &config.profiles[name];
        rows.push((
            name.clone(),
            format!("用户 {}", profile.mode),
            profile.desc.clone(),
        ));
    }

    if rows.is_empty() {
        println!(
            "暂无可用命令（运行 mtrim init 初始化，或在 {} 中编写 profiles）",
            project.user_config_display()
        );
        return Ok(());
    }

    let width = rows
        .iter()
        .map(|(name, _, _)| name.chars().count())
        .max()
        .unwrap_or(0);
    for (name, kind, desc) in rows {
        println!("{name:<width$}  {kind}  {desc}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn lists_user_profiles_and_builtins() {
        let dir = TempDir::new("ls-all");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write(
            "mono-trim.toml",
            "[profiles.win-dev]\nmode = \"black\"\ndesc = \"隐藏 iOS 壳\"\n",
        );
        let project = Project::from_root(dir.path());

        let config = project.load().unwrap();
        assert_eq!(config.sorted_profile_names(), ["win-dev"]);

        // 内置说明必须与 cli 定义一致，避免两处漂移
        assert!(
            cli::BUILTIN_DESCRIPTIONS
                .iter()
                .any(|(name, _)| *name == "applyall")
        );
        run(&project, true).unwrap();
        run(&project, false).unwrap();
    }

    #[test]
    fn handles_empty_project() {
        let dir = TempDir::new("ls-empty");
        let project = Project::from_root(dir.path());

        run(&project, false).unwrap();
    }
}
