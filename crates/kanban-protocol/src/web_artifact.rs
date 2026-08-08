//! `kanban serve` 与 Web/Tauri 共享的静态 Web artifact manifest value contract。
//!
//! 这个模块是显式的非-operation contract：它不进入 endpoint、operation、runtime 或
//! surface catalog。manifest 位于构建产物根的 `manifest.json`，自身不列入 `files`，
//! 也不参与 `buildId`。构建 ID 由以下固定、版本化的 canonical preimage 计算：
//!
//! 1. 先写入 domain `kanban-tool:web-artifact-build-id:v1`（同样使用 UTF-8 byte-length
//!    framing）；
//! 2. 依次写入 `formatVersion`（u64 big-endian）、`basePath`、`entrypoint`、
//!    `serverVersion`、`protocolVersion`（文本均为 UTF-8 byte-length framed）。其中
//!    `serverVersion` 是 host 的 numeric `major.minor.patch` 版本，`protocolVersion`
//!    是 API wire generation（当前固定为 `v1`），两者不是同一种版本语义；
//! 3. 写入 files 数量（u64 big-endian），再按严格的 UTF-8 byte lexicographic path 顺序
//!    写入每个 file 的 path（framed）、bytes（u64 big-endian）和 32-byte SHA-256 digest。
//!
//! 所有长度都是 UTF-8 **字节**长度，所有 u64 都是 8 字节 big-endian；preimage 不使用
//! 分隔符，因此字段边界不会因内容变化而歧义。文本字段拒绝 NUL、控制字符和分隔符，
//! path 另外只允许 ASCII `[A-Za-z0-9._/-]+`，并拒绝绝对路径、反斜杠、`.`/`..` 段、
//! 空段及 manifest 自列。

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 当前静态 Web artifact manifest 格式。
pub const WEB_ARTIFACT_FORMAT_VERSION: u32 = 1;
/// `kanban serve` 托管 Web artifact 的固定 base path。
pub const WEB_ARTIFACT_BASE_PATH: &str = "/app/";
/// 默认入口文档。
pub const WEB_ARTIFACT_ENTRYPOINT: &str = "index.html";
/// manifest 在 dist 根的固定路径；它不在 manifest.files 中。
pub const WEB_ARTIFACT_MANIFEST_PATH: &str = "manifest.json";
/// 当前 API wire protocol generation；它不是 protocol crate 的 Cargo package version。
pub const WEB_PROTOCOL_VERSION: &str = "v1";

const BUILD_ID_DOMAIN: &str = "kanban-tool:web-artifact-build-id:v1";
const BUILD_ID_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;

/// dist 中一个被 host 校验的文件摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebArtifactFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// Browser 与 Tauri 共同消费的静态 Web artifact manifest。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebArtifactManifest {
    pub format_version: u32,
    pub base_path: String,
    pub entrypoint: String,
    pub server_version: String,
    pub protocol_version: String,
    pub build_id: String,
    pub files: Vec<WebArtifactFile>,
}

/// manifest value、digest、path 与 inventory 校验错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebArtifactError(String);

impl WebArtifactError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for WebArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for WebArtifactError {}

/// 校验完整 manifest，并确认 `buildId` 与当前 canonical preimage 一致。
pub fn validate_web_artifact_manifest(
    manifest: &WebArtifactManifest,
) -> Result<(), WebArtifactError> {
    if manifest.format_version != WEB_ARTIFACT_FORMAT_VERSION {
        return Err(WebArtifactError::new(format!(
            "不支持的 Web artifact formatVersion: {}",
            manifest.format_version
        )));
    }
    if manifest.base_path != WEB_ARTIFACT_BASE_PATH {
        return Err(WebArtifactError::new(format!(
            "Web artifact basePath 必须为 {WEB_ARTIFACT_BASE_PATH:?}"
        )));
    }
    if manifest.entrypoint != WEB_ARTIFACT_ENTRYPOINT {
        return Err(WebArtifactError::new(format!(
            "Web artifact entrypoint 必须为 {WEB_ARTIFACT_ENTRYPOINT:?}"
        )));
    }
    validate_server_version(&manifest.server_version)?;
    validate_protocol_version(&manifest.protocol_version)?;
    validate_build_id(&manifest.build_id, "buildId")?;
    let expected = web_artifact_build_id_for(
        manifest.format_version,
        &manifest.base_path,
        &manifest.entrypoint,
        &manifest.server_version,
        &manifest.protocol_version,
        &manifest.files,
    )?;
    if manifest.build_id != expected {
        return Err(WebArtifactError::new(format!(
            "Web artifact buildId 不匹配: manifest={}, expected={expected}",
            manifest.build_id
        )));
    }
    Ok(())
}

/// 对固定 manifest value 计算 canonical `sha256:<64 lowercase hex>` build ID。
pub fn web_artifact_build_id_for(
    format_version: u32,
    base_path: &str,
    entrypoint: &str,
    server_version: &str,
    protocol_version: &str,
    files: &[WebArtifactFile],
) -> Result<String, WebArtifactError> {
    let preimage = web_artifact_build_preimage(
        format_version,
        base_path,
        entrypoint,
        server_version,
        protocol_version,
        files,
    )?;
    let digest = Sha256::digest(preimage);
    Ok(format!("{BUILD_ID_PREFIX}{digest:x}"))
}

/// 暴露 canonical preimage，供跨语言 known-vector 和 host 诊断测试使用。
pub fn web_artifact_build_preimage(
    format_version: u32,
    base_path: &str,
    entrypoint: &str,
    server_version: &str,
    protocol_version: &str,
    files: &[WebArtifactFile],
) -> Result<Vec<u8>, WebArtifactError> {
    validate_build_inputs(
        format_version,
        base_path,
        entrypoint,
        server_version,
        protocol_version,
        files,
    )?;

    let mut preimage = Vec::new();
    append_text(&mut preimage, BUILD_ID_DOMAIN)?;
    append_u64(&mut preimage, u64::from(format_version));
    append_text(&mut preimage, base_path)?;
    append_text(&mut preimage, entrypoint)?;
    append_text(&mut preimage, server_version)?;
    append_text(&mut preimage, protocol_version)?;
    append_u64(
        &mut preimage,
        u64::try_from(files.len()).map_err(|_| WebArtifactError::new("files 数量溢出"))?,
    );
    for file in files {
        append_text(&mut preimage, &file.path)?;
        append_u64(&mut preimage, file.bytes);
        let digest = decode_sha256(&file.sha256, "file.sha256")?;
        preimage.extend_from_slice(&digest);
    }
    Ok(preimage)
}

fn validate_build_inputs(
    format_version: u32,
    base_path: &str,
    entrypoint: &str,
    server_version: &str,
    protocol_version: &str,
    files: &[WebArtifactFile],
) -> Result<(), WebArtifactError> {
    if format_version != WEB_ARTIFACT_FORMAT_VERSION {
        return Err(WebArtifactError::new(format!(
            "不支持的 Web artifact formatVersion: {format_version}"
        )));
    }
    if base_path != WEB_ARTIFACT_BASE_PATH {
        return Err(WebArtifactError::new(format!(
            "Web artifact basePath 必须为 {WEB_ARTIFACT_BASE_PATH:?}"
        )));
    }
    if entrypoint != WEB_ARTIFACT_ENTRYPOINT {
        return Err(WebArtifactError::new(format!(
            "Web artifact entrypoint 必须为 {WEB_ARTIFACT_ENTRYPOINT:?}"
        )));
    }
    validate_server_version(server_version)?;
    validate_protocol_version(protocol_version)?;
    validate_files(files)?;
    Ok(())
}

fn validate_server_version(value: &str) -> Result<(), WebArtifactError> {
    let field = "serverVersion";
    if value.is_empty() || value.trim() != value || !value.is_ascii() {
        return Err(WebArtifactError::new(format!(
            "{field} 必须是无空白 ASCII major.minor.patch 版本"
        )));
    }
    if value
        .bytes()
        .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(WebArtifactError::new(format!("{field} 不得包含控制字符")));
    }
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(WebArtifactError::new(format!(
            "{field} 必须是 major.minor.patch 数字三段版本"
        )));
    }
    Ok(())
}

fn validate_protocol_version(value: &str) -> Result<(), WebArtifactError> {
    if value != WEB_PROTOCOL_VERSION {
        return Err(WebArtifactError::new(format!(
            "protocolVersion 必须精确为 {WEB_PROTOCOL_VERSION:?}，它表示 API wire generation"
        )));
    }
    Ok(())
}

fn validate_build_id(value: &str, field: &str) -> Result<(), WebArtifactError> {
    decode_sha256(value, field).map(|_| ())
}

fn validate_files(files: &[WebArtifactFile]) -> Result<(), WebArtifactError> {
    if files.is_empty() {
        return Err(WebArtifactError::new("Web artifact files 不得为空"));
    }
    let mut previous: Option<&[u8]> = None;
    let mut has_entrypoint = false;
    for file in files {
        validate_path(&file.path)?;
        if file.path == WEB_ARTIFACT_ENTRYPOINT {
            has_entrypoint = true;
        }
        decode_sha256(&file.sha256, "file.sha256")?;
        if let Some(previous) = previous {
            match previous.cmp(file.path.as_bytes()) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(WebArtifactError::new(format!(
                        "Web artifact files 存在重复 path: {}",
                        file.path
                    )));
                }
                std::cmp::Ordering::Greater => {
                    return Err(WebArtifactError::new(
                        "Web artifact files 必须按 UTF-8 byte lexicographic path 排序",
                    ));
                }
            }
        }
        previous = Some(file.path.as_bytes());
    }
    if !has_entrypoint {
        return Err(WebArtifactError::new(format!(
            "Web artifact files 必须包含 {WEB_ARTIFACT_ENTRYPOINT}"
        )));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), WebArtifactError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path == WEB_ARTIFACT_MANIFEST_PATH
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-'))
    {
        return Err(WebArtifactError::new(format!(
            "非法 Web artifact path: {path:?}"
        )));
    }
    let segments = path.split('/').collect::<Vec<_>>();
    if segments
        .iter()
        .any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
    {
        return Err(WebArtifactError::new(format!(
            "Web artifact path 不得包含空、`.` 或 `..` 段: {path:?}"
        )));
    }
    Ok(())
}

fn append_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn append_text(output: &mut Vec<u8>, value: &str) -> Result<(), WebArtifactError> {
    let bytes = value.as_bytes();
    let length = u64::try_from(bytes.len()).map_err(|_| WebArtifactError::new("文本长度溢出"))?;
    append_u64(output, length);
    output.extend_from_slice(bytes);
    Ok(())
}

fn decode_sha256(value: &str, field: &str) -> Result<[u8; 32], WebArtifactError> {
    if !value.starts_with(BUILD_ID_PREFIX)
        || value.len() != BUILD_ID_PREFIX.len() + SHA256_HEX_LENGTH
        || !value[BUILD_ID_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(WebArtifactError::new(format!(
            "{field} 必须是 sha256:<64 lowercase hex>"
        )));
    }
    let mut digest = [0; 32];
    for (index, pair) in value.as_bytes()[BUILD_ID_PREFIX.len()..]
        .chunks_exact(2)
        .enumerate()
    {
        digest[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
    }
    Ok(digest)
}

fn hex_value(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("validated lowercase hex"),
    }
}

impl FromStr for WebArtifactManifest {
    type Err = WebArtifactError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let manifest = serde_json::from_str(value).map_err(|error| {
            WebArtifactError::new(format!("解析 Web artifact manifest 失败: {error}"))
        })?;
        validate_web_artifact_manifest(&manifest)?;
        Ok(manifest)
    }
}
