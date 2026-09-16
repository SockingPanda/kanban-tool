//! MCP 自定义 URI。标识经过一次解码，不接受文件路径或网络 URL。

use rmcp::ErrorData as McpError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum KanbanUri {
    Instructions,
    Runtime,
    Board(String),
    Task(String),
}

impl KanbanUri {
    pub(crate) fn parse(uri: &str) -> Result<Self, McpError> {
        if uri.len() > 2048 || !uri.is_ascii() || uri.contains(['?', '#', '\\']) {
            return Err(invalid_uri());
        }
        let rest = uri.strip_prefix("kanban://").ok_or_else(invalid_uri)?;
        let segments: Vec<_> = rest.split('/').collect();
        let parsed = match segments.as_slice() {
            ["server", "instructions"] => Self::Instructions,
            ["server", "runtime"] => Self::Runtime,
            ["boards", board] => Self::Board(decode_segment(board)?),
            ["tasks", task] => {
                let id = decode_segment(task)?;
                validate_task_id(&id)?;
                Self::Task(id)
            }
            _ => return Err(invalid_uri()),
        };
        if parsed.to_uri() != uri {
            return Err(McpError::invalid_params(
                "资源 URI 必须使用规范形式及大写百分号编码",
                None,
            ));
        }
        Ok(parsed)
    }

    pub(crate) fn to_uri(&self) -> String {
        match self {
            Self::Instructions => "kanban://server/instructions".into(),
            Self::Runtime => "kanban://server/runtime".into(),
            Self::Board(board) => format!("kanban://boards/{}", encode_segment(board)),
            Self::Task(task) => format!("kanban://tasks/{}", encode_segment(task)),
        }
    }
}

pub(crate) fn validate_task_id(id: &str) -> Result<(), McpError> {
    if !id.starts_with("t_")
        || id.len() < 3
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(McpError::invalid_params(
            "资源要求全局 t_... task_id；请先用 task_list/task_show 解析 board-local selector",
            None,
        ));
    }
    Ok(())
}

fn encode_segment(segment: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::new();
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 15) as usize]));
        }
    }
    encoded
}

fn decode_segment(segment: &str) -> Result<String, McpError> {
    let mut decoded = Vec::new();
    let mut bytes = segment.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next().and_then(hex).ok_or_else(invalid_uri)?;
            let low = bytes.next().and_then(hex).ok_or_else(invalid_uri)?;
            decoded.push((high << 4) | low);
        } else {
            decoded.push(byte);
        }
    }
    let text = String::from_utf8(decoded).map_err(|_| invalid_uri())?;
    if text.is_empty()
        || text.len() > 256
        || text == "."
        || text == ".."
        || text.trim() != text
        || text.contains(['/', '\\'])
        || text.chars().any(char::is_control)
    {
        return Err(invalid_uri());
    }
    Ok(text)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn invalid_uri() -> McpError {
    McpError::invalid_params("资源不存在或 URI 无效", None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_board_has_one_canonical_encoding() {
        let resource = KanbanUri::Board("中文项目".into());
        assert_eq!(KanbanUri::parse(&resource.to_uri()).unwrap(), resource);
        assert!(KanbanUri::parse("kanban://boards/中文项目").is_err());
        assert!(KanbanUri::parse("kanban://boards/%64efault").is_err());
    }

    #[test]
    fn rejects_paths_invalid_utf8_fragments_and_encoded_separators() {
        for uri in [
            "file:///etc/passwd",
            "https://example.com/secret",
            "kanban://boards/../tasks/x",
            "kanban://boards/%2E%2E",
            "kanban://boards/a%2Fb",
            "kanban://boards/a%5Cb",
            "kanban://boards/a%00",
            "kanban://boards/%C0%AF",
            "kanban://boards/%",
            "kanban://boards/default?q=1",
            "kanban://boards/default#fragment",
            "kanban://server/unknown",
        ] {
            assert!(KanbanUri::parse(uri).is_err(), "{uri}");
        }
    }

    #[test]
    fn task_resources_use_global_ids_without_a_hidden_board() {
        assert_eq!(
            KanbanUri::parse("kanban://tasks/t_123").unwrap(),
            KanbanUri::Task("t_123".into())
        );
        for uri in [
            "kanban://tasks/123",
            "kanban://tasks/%23123",
            "kanban://tasks/other%23123",
        ] {
            assert!(KanbanUri::parse(uri).is_err());
        }
    }
}
