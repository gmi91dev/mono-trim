//! 终端进度条（零依赖）：仅在交互式终端下绘制，重定向时自动静默。

use std::io::Write;
use std::time::{Duration, Instant};

/// 刷新间隔，避免高频重绘拖慢下载
const THROTTLE: Duration = Duration::from_millis(100);
/// 进度条宽度（字符数）
const BAR_WIDTH: usize = 30;

/// 可复用的下载进度条
pub struct ProgressBar {
    label: String,
    total: Option<u64>,
    enabled: bool,
    last_draw: Instant,
    max_len: usize,
}

impl ProgressBar {
    pub fn new(label: impl Into<String>, total: Option<u64>) -> Self {
        Self {
            label: label.into(),
            total,
            enabled: crate::util::is_interactive(),
            // 首次绘制不节流
            last_draw: Instant::now() - THROTTLE,
            max_len: 0,
        }
    }

    /// 更新进度（内部按 100ms 节流）
    pub fn advance(&mut self, done: u64) {
        if !self.enabled || self.last_draw.elapsed() < THROTTLE {
            return;
        }
        self.last_draw = Instant::now();
        self.draw(&render_progress(&self.label, done, self.total));
    }

    /// 完成：补满进度条并换行
    pub fn finish(&mut self) {
        if !self.enabled {
            return;
        }
        let done = self.total.unwrap_or(0);
        let line = render_progress(&self.label, done, self.total);
        self.draw(&line);
        println!();
    }

    fn draw(&mut self, line: &str) {
        let width = line.chars().count();
        self.max_len = self.max_len.max(width);
        // 用空格覆盖上一次更长的输出，避免残留字符
        let padding = " ".repeat(self.max_len - width);
        print!("\r{line}{padding}");
        let _ = std::io::stdout().flush();
    }
}

/// 渲染一行进度文本（便于单元测试）
pub fn render_progress(label: &str, done: u64, total: Option<u64>) -> String {
    match total.filter(|value| *value > 0) {
        Some(total) => {
            let ratio = (done as f64 / total as f64).clamp(0.0, 1.0);
            let filled = (ratio * BAR_WIDTH as f64).round() as usize;
            format!(
                "{label} [{}{}] {:>3}%  {}/{}",
                "#".repeat(filled),
                "-".repeat(BAR_WIDTH - filled),
                (ratio * 100.0).round() as u32,
                human_bytes(done),
                human_bytes(total)
            )
        }
        // 服务端没有给出 Content-Length 时退化为"已下载量"
        None => format!("{label} {} 已下载", human_bytes(done)),
    }
}

fn human_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let value = bytes as f64;
    if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.0} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_percentage_and_bar() {
        let line = render_progress("下载", 512, Some(1024));
        assert!(line.contains(" 50%"), "{line}");
        assert!(line.contains("512 B/1 KiB"), "{line}");
        assert_eq!(line.matches('#').count(), BAR_WIDTH / 2);

        let full = render_progress("下载", 2048, Some(1024));
        assert!(full.contains("100%"), "{full}");
        assert_eq!(
            full.matches('#').count(),
            BAR_WIDTH,
            "超额下载时不应超过 100%"
        );
    }

    #[test]
    fn falls_back_to_downloaded_bytes_without_total() {
        let line = render_progress("下载", 1536, None);
        assert_eq!(line, "下载 2 KiB 已下载");
    }

    #[test]
    fn formats_human_readable_sizes() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2 KiB");
        assert_eq!(human_bytes(3 * 1024 * 1024), "3.0 MiB");
    }

    #[test]
    fn progress_is_silent_when_not_a_terminal() {
        // 测试环境 stdout 被重定向：绘制不应产生内容（这里只验证 guard 状态）
        let mut bar = ProgressBar::new("下载", Some(100));
        assert!(!bar.enabled);
        bar.advance(50);
        bar.finish();
    }
}
