//! 测试辅助：不引入第三方依赖的临时目录。
//!
//! 仅用于单元测试，`main.rs` 中以 `#[cfg(test)]` 声明。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 进程内唯一的临时目录，`Drop` 时自动清理
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mono-trim-test-{}-{label}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("创建测试临时目录失败");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 写入文件，自动创建父目录
    pub fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path.join(relative)).unwrap()
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.path.join(relative).exists()
    }
}

/// 写入 `.mono-trim/config.toml` 并返回其路径
pub fn write_config(dir: &TempDir, body: &str) -> PathBuf {
    dir.write(
        &format!("{}/{}", crate::config::TOOL_DIR, crate::config::CONFIG_NAME),
        body,
    )
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 记录调用参数并返回预置输出的假 git，用于验证命令编排
pub struct FakeGit {
    responses: std::cell::RefCell<Vec<(String, crate::git::GitOutput)>>,
    calls: std::cell::RefCell<Vec<String>>,
}

impl FakeGit {
    pub fn new() -> Self {
        Self {
            responses: std::cell::RefCell::new(Vec::new()),
            calls: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// 预置一条命令的预期输出，按调用顺序消费
    pub fn on(self, args: &str, output: crate::git::GitOutput) -> Self {
        self.responses.borrow_mut().push((args.to_string(), output));
        self
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl crate::git::GitRunner for FakeGit {
    fn run(&self, args: &[&str]) -> crate::Result<crate::git::GitOutput> {
        let key = args.join(" ");
        self.calls.borrow_mut().push(key.clone());

        let mut responses = self.responses.borrow_mut();
        let index = responses
            .iter()
            .position(|(expected, _)| *expected == key)
            .ok_or_else(|| format!("假 git 未预置命令：{key}"))?;
        Ok(responses.remove(index).1)
    }
}
