//! 版本号比较：配置里记录的应用版本与当前 CLI 版本的比较。

/// 解析 `x.y.z`（允许 `v` 前缀），失败返回 None
fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let text = version.trim().trim_start_matches('v');
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_valid(version: &str) -> bool {
    parse(version).is_some()
}

/// `a` 是否比 `b` 更新；无法解析时退化为字符串不相等即视为不同
pub fn is_newer(a: &str, b: &str) -> bool {
    match (parse(a), parse(b)) {
        (Some(a), Some(b)) => a > b,
        _ => a != b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_semver() {
        assert!(is_valid("0.2.0"));
        assert!(is_valid("v1.10.3"));
        assert!(!is_valid("0.2"));
        assert!(!is_valid("0.2.0.1"));
        assert!(!is_valid("abc"));
    }

    #[test]
    fn compares_numerically_not_lexically() {
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(!is_newer("0.9.0", "0.10.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(is_newer("v1.0.1", "1.0.0"));
    }
}
