use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub download_url: String,
    pub body: String,
}

pub fn check_latest_release() -> Result<Option<ReleaseInfo>> {
    let current = env!("CARGO_PKG_VERSION");
    let url = "https://api.github.com/repos/ccccyccccc/fastZIP/releases/latest";
    let user_agent = format!("FastZIP/{}", current);

    let response = ureq::get(url)
        .set("User-Agent", &user_agent)
        .set("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| "Failed to contact GitHub for update check")?;

    let body: String = response
        .into_string()
        .with_context(|| "Failed to read GitHub response")?;

    let tag = extract_json_string(&body, "tag_name")
        .unwrap_or("0.0.0")
        .trim_start_matches('v')
        .to_string();

    if !is_newer(&tag, current) {
        return Ok(None);
    }

    let download_url = extract_json_string(&body, "html_url")
        .unwrap_or("https://github.com/ccccyccccc/fastZIP/releases/latest")
        .to_string();

    let changelog = extract_json_string(&body, "body").unwrap_or("").to_string();

    Ok(Some(ReleaseInfo {
        version: tag,
        download_url,
        body: changelog,
    }))
}

fn extract_json_string<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let search = format!("\"{}\"", key);
    let after_key = json.find(&search)?;
    let after = &json[after_key + search.len()..];
    let after_colon = after.find(':')?;
    let start = after[after_colon + 1..].find('"')? + after_colon + 1;
    let val_start = start + 1;
    let mut escaping = false;
    for (i, ch) in after[val_start..].char_indices() {
        if escaping {
            escaping = false;
            continue;
        }
        if ch == '\\' {
            escaping = true;
            continue;
        }
        if ch == '"' {
            return Some(&after[val_start..val_start + i]);
        }
    }
    None
}

fn is_newer(latest: &str, current: &str) -> bool {
    parse_semver(latest) > parse_semver(current)
}

fn parse_semver(v: &str) -> (u32, u32, u32) {
    let mut parts = v
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty());
    let major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"tag_name": "v0.2.0", "html_url": "https://example.com/releases/tag/v0.2.0", "body": "Release notes - Feature A - Bug fix B"}"#;
        assert_eq!(extract_json_string(json, "tag_name"), Some("v0.2.0"));
        assert_eq!(
            extract_json_string(json, "html_url"),
            Some("https://example.com/releases/tag/v0.2.0")
        );
        assert_eq!(
            extract_json_string(json, "body"),
            Some("Release notes - Feature A - Bug fix B")
        );
    }

    #[test]
    fn test_semver_comparison() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
    }

    #[test]
    fn test_strips_v_prefix() {
        assert!(is_newer("v0.2.0", "0.1.0"));
    }
}
