//! `show`：打印两份配置文件的位置与是否存在。

use crate::Result;
use crate::config::Project;

pub fn run(project: &Project) -> Result<()> {
    let system = project.system_config_display();
    let user = project.user_config_display();
    println!(
        "系统配置  {system}  {}",
        status(project.system_config().exists())
    );
    println!(
        "用户配置  {user}  {}",
        status(project.user_config().exists())
    );
    Ok(())
}

fn status(exists: bool) -> &'static str {
    if exists {
        "（存在）"
    } else {
        "（不存在）"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn reports_existing_config_files() {
        let dir = TempDir::new("show");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        let project = Project::from_root(dir.path());

        assert!(project.system_config().exists());
        assert!(!project.user_config().exists());
        assert_eq!(status(true), "（存在）");
        assert_eq!(status(false), "（不存在）");

        run(&project).unwrap();
    }
}
