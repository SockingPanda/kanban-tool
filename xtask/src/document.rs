use xtask::ToolResult;

pub(crate) fn section_body<'a>(text: &'a str, heading: &str) -> ToolResult<&'a str> {
    let (_, body) = text.split_once(heading).ok_or_else(|| {
        std::io::Error::other(format!("根 AGENTS.md 缺少必要 section: {heading}"))
    })?;
    Ok(body.split_once("\n## ").map_or(body, |(body, _)| body))
}

pub(crate) fn section_contains_bullet(section: &str, needle: &str) -> bool {
    section
        .lines()
        .any(|line| line.trim_start().starts_with("- ") && line.contains(needle))
}

pub(crate) fn markdown_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("](") {
        let after = &remaining[start + 2..];
        let Some(end) = after.find(')') else { break };
        let raw = after[..end].trim();
        let raw = raw
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
            .unwrap_or(raw);
        if let Some(target) = raw.split_whitespace().next() {
            targets.push(target.trim_matches('"').to_owned());
        }
        remaining = &after[end + 1..];
    }
    targets
}

pub(crate) fn is_external_link(target: &str) -> bool {
    target.starts_with("//")
        || target.starts_with('/')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("file:")
}
