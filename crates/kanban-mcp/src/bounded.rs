//! 限制序列化后的 JSON 大小；不额外分配同等大小的临时字符串。
//! 此处限制参数/结果，不是 stdio 入站帧解析器的内存上限。

use std::io::{self, Write};

use rmcp::ErrorData as McpError;
use serde::Serialize;

struct Counter {
    remaining: usize,
    used: usize,
    exceeded: bool,
}

impl Write for Counter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.remaining {
            self.exceeded = true;
            return Err(io::Error::other("encoded JSON exceeds limit"));
        }
        self.remaining -= buffer.len();
        self.used += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// 成功时返回准确字节数；超过限额时返回 None。
pub(crate) fn json_size<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
) -> Result<Option<usize>, McpError> {
    let mut counter = Counter {
        remaining: limit,
        used: 0,
        exceeded: false,
    };
    match serde_json::to_writer(&mut counter, value) {
        Ok(()) => Ok(Some(counter.used)),
        Err(_) if counter.exceeded => Ok(None),
        Err(_) => Err(McpError::internal_error("MCP 结果 JSON 编码失败", None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_utf8_and_escaping_not_characters() {
        let value = serde_json::json!({"value": "中文\n\""});
        let length = serde_json::to_vec(&value).unwrap().len();
        assert_eq!(json_size(&value, length).unwrap(), Some(length));
        assert_eq!(json_size(&value, length - 1).unwrap(), None);
    }

    #[test]
    fn zero_budget_cannot_encode_even_null() {
        assert_eq!(json_size(&serde_json::Value::Null, 0).unwrap(), None);
    }
}
