//! 排序后的静态目录分页。游标只用于定位，不代表授权。

use rmcp::ErrorData as McpError;
use serde::Serialize;

use crate::bounded::json_size;

pub(crate) const ENVELOPE_RESERVE: usize = 4096;

pub(crate) fn page<T: Clone + Serialize>(
    items: &[T],
    cursor: Option<&str>,
    scope: &str,
    stamp: &str,
    count: usize,
    max_bytes: usize,
) -> Result<(Vec<T>, Option<String>), McpError> {
    if count == 0 {
        return Err(McpError::internal_error("分页大小不能为 0", None));
    }
    let prefix = format!("mcp1.{scope}.{stamp}.");
    let start = match cursor {
        None => 0,
        Some(cursor) => {
            let number = cursor
                .strip_prefix(&prefix)
                .filter(|_| cursor.len() <= 128)
                .ok_or_else(invalid_cursor)?;
            let index: usize = number.parse().map_err(|_| invalid_cursor())?;
            if index == 0 || index >= items.len() || index.to_string() != number {
                return Err(invalid_cursor());
            }
            index
        }
    };
    let mut remaining = max_bytes
        .checked_sub(ENVELOPE_RESERVE)
        .ok_or_else(|| McpError::internal_error("目录响应预算过小", None))?;
    let mut selected = Vec::new();
    for item in items.iter().skip(start).take(count) {
        match json_size(item, remaining.saturating_sub(1))? {
            Some(bytes) => {
                remaining -= bytes + 1;
                selected.push(item.clone());
            }
            None if selected.is_empty() => {
                return Err(McpError::internal_error(
                    "单个目录项超过响应预算，请调整 MCP 配置",
                    None,
                ));
            }
            None => break,
        }
    }
    let end = start + selected.len();
    let next = (end < items.len()).then(|| format!("{prefix}{end}"));
    Ok((selected, next))
}

fn invalid_cursor() -> McpError {
    McpError::invalid_params("MCP 分页游标无效或已过期，请从第一页重读", None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_is_complete_and_namespace_scoped() {
        let items = vec![1, 2, 3, 4, 5];
        let (first, cursor) = page(&items, None, "tools", "abc", 2, 8192).unwrap();
        assert_eq!(first, vec![1, 2]);
        let (second, _) = page(&items, cursor.as_deref(), "tools", "abc", 2, 8192).unwrap();
        assert_eq!(second, vec![3, 4]);
        assert!(page(&items, cursor.as_deref(), "resources", "abc", 2, 8192).is_err());
        assert!(page(&items, cursor.as_deref(), "tools", "changed", 2, 8192).is_err());
    }

    #[test]
    fn rejects_invalid_indices_and_spelling() {
        for cursor in [
            "mcp1.tools.abc.0",
            "mcp1.tools.abc.9",
            "mcp1.tools.abc.-1",
            "mcp1.tools.abc.01",
            "mcp1.tools.abc.+1",
            "garbage",
        ] {
            assert!(page(&[1, 2], Some(cursor), "tools", "abc", 2, 8192).is_err());
        }
    }

    #[test]
    fn zero_page_size_never_emits_a_nonadvancing_cursor() {
        assert!(page(&[1], None, "tools", "abc", 0, 8192).is_err());
    }

    #[test]
    fn byte_budget_shortens_page_without_truncating_item() {
        let items = vec!["x".repeat(1500), "y".repeat(1500)];
        let (first, next) = page(&items, None, "tools", "abc", 10, 6100).unwrap();
        assert_eq!(first.len(), 1);
        assert!(next.is_some());
        assert!(page(&items, None, "tools", "abc", 10, 4200).is_err());
    }
}
