//! 裁剪相关命令：用户自定义 profile 与 `applyall`。

use crate::Result;
use crate::config::Project;
use crate::git::{GitRunner, git_ok, sparse_checkout_path, sparse_enabled};
use crate::sparse::{Action, INCLUDE_ALL, build_plan};
use crate::util::write_file;

/// 执行用户配置中的自定义 profile
pub fn profile(project: &Project, name: &str, git: &dyn GitRunner) -> Result<()> {
    if !project.is_initialized() {
        return Err(format!(
            "未发现系统配置 {}，请先运行 mtrim init",
            project.system_config_display()
        ));
    }

    let config = project.load()?;
    if let Some(hint) = config.version_hint() {
        println!("⚠️  {hint}");
    }

    let profile = config
        .profiles
        .get(name)
        .ok_or_else(|| format!("配置中没有 '{name}' 命令。运行 mtrim ls 查看可用命令。"))?;

    println!("⏳ 正在执行自定义命令 [{name}] ({}) ...", profile.desc);
    // 先完整校验并生成计划，任何非法配置都在触碰工作空间之前拦截
    let plan = build_plan(profile)?;
    for action in &plan.actions {
        match action {
            Action::Exclude(target) => println!("   🚫 [黑名单排除] -> {target}"),
            Action::Keep(target) => println!("   ✅ [白名单仅保留] -> {target}"),
        }
    }

    git.run_inherit(&["sparse-checkout", "init", "--no-cone"])?;
    write_file(&sparse_checkout_path(git)?, &plan.patterns)?;

    println!("⏳ 正在更新工作区（大仓库需要按需下载文件，git 会显示进度）...");
    let started = std::time::Instant::now();
    git.run_inherit(&["sparse-checkout", "reapply"])?;

    println!(
        "✨ 工作空间拓扑裁剪成功（用时 {:.1}s）！",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// 应用全量规则，恢复完整工作空间（不依赖项目配置）
pub fn applyall(git: &dyn GitRunner) -> Result<()> {
    crate::git::ensure_repo(git)?;
    println!("⏳ 正在恢复全量工作空间...");

    if !sparse_enabled(git)? {
        println!("✅ 当前已是全量工作空间");
        return Ok(());
    }

    // 优先使用官方 disable 彻底关闭稀疏检出，失败则退回全量规则
    if git_ok(git, &["sparse-checkout", "disable"]).is_err() {
        write_file(&sparse_checkout_path(git)?, &format!("{INCLUDE_ALL}\n"))?;
        git_ok(git, &["sparse-checkout", "reapply"])?;
    }

    println!("✅ 已恢复全量工作空间");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;
    use crate::git::GitOutput;
    use crate::test_support::{FakeGit, TempDir};

    const USER_CONFIG: &str = r#"[profiles.win-dev]
mode = "black"
desc = "隐藏 iOS 壳"
targets = ["apps/ios/"]
"#;

    fn project_with_user_config(dir: &TempDir) -> Project {
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write("mono-trim.toml", USER_CONFIG);
        Project::from_root(dir.path())
    }

    #[test]
    fn unknown_profile_is_rejected() {
        let dir = TempDir::new("apply-unknown");
        let project = project_with_user_config(&dir);
        let git = FakeGit::new();

        let err = profile(&project, "nope", &git).unwrap_err();

        assert!(err.contains("配置中没有 'nope' 命令"), "{err}");
        assert!(git.calls().is_empty(), "不应调用 git");
    }

    #[test]
    fn requires_initialized_project() {
        let dir = TempDir::new("apply-uninitialized");
        let project = Project::from_root(dir.path());

        let err = profile(&project, "win-dev", &FakeGit::new()).unwrap_err();

        assert!(err.contains("mtrim init"), "{err}");
    }

    #[test]
    fn invalid_mode_fails_before_touching_workspace() {
        let dir = TempDir::new("apply-mode");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write(
            "mono-trim.toml",
            "[profiles.bad]\nmode = \"gray\"\ntargets = []\n",
        );
        let project = Project::from_root(dir.path());
        let git = FakeGit::new();

        let err = profile(&project, "bad", &git).unwrap_err();

        assert!(err.contains("未知的 mode"), "{err}");
        assert!(git.calls().is_empty());
    }

    #[test]
    fn writes_plan_then_reapplies() {
        let dir = TempDir::new("apply-write");
        let project = project_with_user_config(&dir);
        let sparse_path = dir.write(".git/info/sparse-checkout", "");
        let git = FakeGit::new()
            .on("sparse-checkout init --no-cone", GitOutput::ok(""))
            .on(
                "rev-parse --git-path info/sparse-checkout",
                GitOutput::ok(sparse_path.to_string_lossy()),
            )
            .on("sparse-checkout reapply", GitOutput::ok(""));

        profile(&project, "win-dev", &git).unwrap();

        assert_eq!(
            std::fs::read_to_string(&sparse_path).unwrap(),
            "/*\n!/apps/ios/\n"
        );
        assert_eq!(git.calls().len(), 3);
    }

    #[test]
    fn applyall_is_noop_when_already_full() {
        let git = FakeGit::new()
            .on("rev-parse --is-inside-work-tree", GitOutput::ok("true"))
            .on("config --bool core.sparseCheckout", GitOutput::ok("false"));

        applyall(&git).unwrap();

        assert_eq!(
            git.calls(),
            [
                "rev-parse --is-inside-work-tree",
                "config --bool core.sparseCheckout"
            ]
        );
    }

    #[test]
    fn applyall_disables_sparse_checkout() {
        let git = FakeGit::new()
            .on("rev-parse --is-inside-work-tree", GitOutput::ok("true"))
            .on("config --bool core.sparseCheckout", GitOutput::ok("true"))
            .on("sparse-checkout disable", GitOutput::ok(""));

        applyall(&git).unwrap();

        assert_eq!(git.calls().last().unwrap(), "sparse-checkout disable");
    }

    #[test]
    fn applyall_falls_back_to_full_patterns() {
        let dir = TempDir::new("applyall-fallback");
        let sparse_path = dir.write(".git/info/sparse-checkout", "old");
        let git = FakeGit::new()
            .on("rev-parse --is-inside-work-tree", GitOutput::ok("true"))
            .on("config --bool core.sparseCheckout", GitOutput::ok("true"))
            .on("sparse-checkout disable", GitOutput::fail("nope"))
            .on(
                "rev-parse --git-path info/sparse-checkout",
                GitOutput::ok(sparse_path.to_string_lossy()),
            )
            .on("sparse-checkout reapply", GitOutput::ok(""));

        applyall(&git).unwrap();

        assert_eq!(std::fs::read_to_string(&sparse_path).unwrap(), "/*\n");
    }

    #[test]
    fn profile_builder_is_reused_for_both_modes() {
        // 防止 build_plan 与 apply 之间的契约漂移
        let profile = Profile {
            mode: "white".into(),
            desc: String::new(),
            targets: vec!["rust/".into()],
        };
        let plan = build_plan(&profile).unwrap();
        assert!(plan.patterns.contains("/rust/"));
    }
}
