//! 进程启动配置。配置文件路径只由启动者提供，MCP 请求不能更改配置。

use std::{collections::BTreeSet, env, fs::File, io::Read, net::IpAddr};

use anyhow::{Context, bail, ensure};
use kanban_client::DEFAULT_SERVER_URL;
use serde::{Deserialize, Serialize};

const CONFIG_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Profile {
    #[default]
    Work,
    ReadOnly,
    All,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Limits {
    pub(crate) max_in_flight: usize,
    pub(crate) timeout_ms: u64,
    pub(crate) max_arguments_bytes: usize,
    pub(crate) max_result_bytes: usize,
    pub(crate) page_size: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_in_flight: 8,
            timeout_ms: 30_000,
            max_arguments_bytes: 64 * 1024,
            max_result_bytes: 256 * 1024,
            page_size: 32,
        }
    }
}

impl Limits {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            (1..=64).contains(&self.max_in_flight),
            "max_in_flight 必须在 1..=64"
        );
        ensure!(
            (1_000..=30_000).contains(&self.timeout_ms),
            "timeout_ms 必须在 1000..=30000"
        );
        ensure!(
            (1_024..=1024 * 1024).contains(&self.max_arguments_bytes),
            "max_arguments_bytes 必须在 1024..=1048576"
        );
        ensure!(
            (16 * 1024..=4 * 1024 * 1024).contains(&self.max_result_bytes),
            "max_result_bytes 必须在 16384..=4194304"
        );
        ensure!(
            (1..=32).contains(&self.page_size),
            "page_size 必须在 1..=32"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub(crate) config_version: u32,
    pub(crate) server_url: String,
    pub(crate) actor: String,
    pub(crate) default_board: String,
    pub(crate) profile: Profile,
    pub(crate) disabled_tools: BTreeSet<String>,
    pub(crate) limits: Limits,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: 1,
            server_url: DEFAULT_SERVER_URL.to_owned(),
            actor: "mcp".to_owned(),
            default_board: "default".to_owned(),
            profile: Profile::Work,
            disabled_tools: BTreeSet::new(),
            limits: Limits::default(),
        }
    }
}

impl Config {
    pub(crate) fn load() -> anyhow::Result<Self> {
        let text = match read_env("KANBAN_MCP_CONFIG")? {
            Some(path) => {
                ensure!(!path.is_empty(), "KANBAN_MCP_CONFIG 不能为空");
                let mut text = String::new();
                File::open(&path)
                    .context("无法读取 KANBAN_MCP_CONFIG 指向的文件")?
                    .take((CONFIG_BYTES + 1) as u64)
                    .read_to_string(&mut text)
                    .context("MCP 配置必须是 UTF-8 JSON")?;
                ensure!(text.len() <= CONFIG_BYTES, "MCP 配置文件超过 64 KiB");
                Some(text)
            }
            None => None,
        };
        Self::from_sources(text.as_deref(), read_env)
    }

    /// 注入环境读取器，测试不修改进程全局环境。
    fn from_sources(
        text: Option<&str>,
        mut environment: impl FnMut(&str) -> anyhow::Result<Option<String>>,
    ) -> anyhow::Result<Self> {
        let mut config = match text {
            Some(text) => serde_json::from_str(text).context("MCP 配置 JSON 无效")?,
            None => Self::default(),
        };
        for (key, target) in [
            ("KANBAN_SERVER_URL", &mut config.server_url),
            ("KANBAN_ACTOR", &mut config.actor),
            ("KB_BOARD", &mut config.default_board),
        ] {
            if let Some(value) = environment(key)? {
                *target = value;
            }
        }
        if let Some(value) = environment("KANBAN_MCP_PROFILE")? {
            config.profile = match value.as_str() {
                "work" => Profile::Work,
                "read_only" => Profile::ReadOnly,
                "all" => Profile::All,
                _ => bail!("KANBAN_MCP_PROFILE 必须为 work、read_only 或 all"),
            };
        }
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.config_version == 1, "不支持的 MCP config_version");
        validate_server_url(&self.server_url)?;
        validate_label(&self.actor, 128, "actor")?;
        validate_label(&self.default_board, 256, "default_board")?;
        for tool in &self.disabled_tools {
            ensure!(valid_tool_name(tool), "disabled_tools 含无效工具名");
        }
        self.limits.validate()
    }
}

fn read_env(key: &str) -> anyhow::Result<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => bail!("环境变量 {key} 必须是 UTF-8"),
    }
}

fn validate_label(value: &str, max: usize, name: &str) -> anyhow::Result<()> {
    ensure!(!value.is_empty() && value.len() <= max, "{name} 长度无效");
    ensure!(
        value.trim() == value && !value.chars().any(char::is_control),
        "{name} 含首尾空白或控制字符"
    );
    Ok(())
}

pub(crate) fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

/// 当前产品只允许本机 host；不接受凭据、路径、查询或远程地址。
fn validate_server_url(value: &str) -> anyhow::Result<()> {
    let authority = value
        .strip_prefix("http://")
        .context("server_url 必须使用本机 http:// gRPC endpoint")?;
    let authority = authority.strip_suffix('/').unwrap_or(authority);
    ensure!(
        !authority
            .chars()
            .any(|c| c.is_whitespace() || c.is_control()),
        "server_url 含空白"
    );
    ensure!(
        !authority.contains(['/', '?', '#', '@', '\\']),
        "server_url 只允许 host:port"
    );
    let (host, port) = authority
        .rsplit_once(':')
        .context("server_url 必须显式指定端口")?;
    ensure!(
        !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()),
        "server_url 端口必须为十进制数字"
    );
    let port: u16 = port.parse().context("server_url 端口无效")?;
    ensure!(port != 0, "server_url 端口不能为 0");
    let loopback = if let Some(host) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        host.parse::<std::net::Ipv6Addr>()
            .is_ok_and(|ip| ip.is_loopback())
    } else {
        !host.contains(['[', ']', ':'])
            && (host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()))
    };
    ensure!(loopback, "MCP 只连接本机 loopback host");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_is_env_then_file_then_default() {
        let config = Config::from_sources(Some(r#"{"actor":"file","profile":"all"}"#), |key| {
            Ok((key == "KANBAN_ACTOR").then(|| "agent-1".to_owned()))
        })
        .unwrap();
        assert_eq!(config.actor, "agent-1");
        assert_eq!(config.profile, Profile::All);
        assert_eq!(config.default_board, "default");
    }

    #[test]
    fn rejects_unknown_keys_and_versions() {
        for text in [
            r#"{"profil":"all"}"#,
            r#"{"config_version":2}"#,
            r#"{"limits":{"page_sze":2}}"#,
        ] {
            assert!(Config::from_sources(Some(text), |_| Ok(None)).is_err());
        }
    }

    #[test]
    fn refuses_remote_credential_and_path_endpoints() {
        for url in [
            "https://example.com:8721",
            "http://example.com:8721",
            "http://127.0.0.1.evil:8721",
            "http://user:secret@127.0.0.1:8721",
            "http://127.0.0.1:8721/foo",
            "http://127.0.0.1:8721?x=1",
            "http://127.0.0.1:0",
            "http://::1:8721",
            "http://[127.0.0.1]:8721",
            "http://127.0.0.1:+8721",
            "http://127.0.0.1:8721\n",
        ] {
            assert!(validate_server_url(url).is_err(), "{url}");
        }
        for url in [
            "http://127.0.0.1:8721",
            "http://localhost:8721/",
            "http://[::1]:8721",
        ] {
            assert!(validate_server_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn zero_and_excessive_limits_are_rejected() {
        for limits in [
            Limits {
                max_in_flight: 0,
                ..Limits::default()
            },
            Limits {
                page_size: 33,
                ..Limits::default()
            },
            Limits {
                timeout_ms: 30_001,
                ..Limits::default()
            },
        ] {
            assert!(limits.validate().is_err());
        }
    }

    #[test]
    fn bad_env_profile_and_empty_actor_do_not_fall_back() {
        assert!(
            Config::from_sources(None, |key| Ok(
                (key == "KANBAN_MCP_PROFILE").then(|| "readonly".into())
            ))
            .is_err()
        );
        assert!(
            Config::from_sources(None, |key| Ok((key == "KANBAN_ACTOR").then(String::new)))
                .is_err()
        );
    }
}
