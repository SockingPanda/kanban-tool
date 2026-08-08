#![doc = include_str!("../README.md")]

//! `kanban serve` 与 Tauri host 复用的 immutable Web artifact filesystem verifier。
//!
//! 该 crate 只拥有从绝对 dist 目录加载、校验并冻结 Web artifact snapshot 的逻辑；它
//! 不提供 HTTP content type、ETag、路由或 package CLI。调用方拿到 [`VerifiedWebArtifact`]
//! 后不需要重新打开磁盘路径，所有 manifest/payload bytes 都保存在 immutable `Arc` 中。
//!
//! 文件系统边界是 cooperative：读取前后会比较 path/fd 的 identity、长度和 link count，
//! 最终快照不会暴露 root 供调用方重开；它不承诺抵抗同 UID 的恶意并发替换，也不引入
//! `openat2` 或 `cap-std`。生产 host 应在切换 artifact 前完成一次完整验证。

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use kanban_protocol::{
    WEB_ARTIFACT_MANIFEST_PATH, WebArtifactFile, WebArtifactManifest, web_artifact_file_from_bytes,
    web_artifact_sha256_for_bytes,
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

#[derive(Debug)]
pub enum WebArtifactVerificationError {
    Io { path: PathBuf, source: io::Error },
    Invalid(String),
}

impl WebArtifactVerificationError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}

impl fmt::Display for WebArtifactVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "读取 Web artifact 路径 {} 失败: {source}",
                    path.display()
                )
            }
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WebArtifactVerificationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

/// 一个已冻结的 payload；bytes 与 descriptor 的 path 始终对应。
#[derive(Debug, Clone)]
pub struct VerifiedWebArtifactPayload {
    descriptor: WebArtifactFile,
    bytes: Arc<[u8]>,
}

impl VerifiedWebArtifactPayload {
    /// manifest 中的相对 payload path。
    pub fn path(&self) -> &str {
        &self.descriptor.path
    }

    /// manifest 中的完整文件摘要。
    pub fn descriptor(&self) -> &WebArtifactFile {
        &self.descriptor
    }

    /// 返回 immutable snapshot bytes 的只读视图。
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// 克隆 immutable bytes handle，供后续异步/HTTP adapter 持有。
    pub fn bytes_arc(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }
}

/// 已通过完整 manifest、exact tree 与 digest 校验的 immutable artifact snapshot。
#[derive(Debug, Clone)]
pub struct VerifiedWebArtifact {
    manifest: WebArtifactManifest,
    manifest_bytes: Arc<[u8]>,
    manifest_sha256: String,
    payloads: Vec<VerifiedWebArtifactPayload>,
}

impl VerifiedWebArtifact {
    /// 已解析且通过 protocol validator 的 manifest value。
    pub fn manifest(&self) -> &WebArtifactManifest {
        &self.manifest
    }

    /// 磁盘上读取到的原始 `manifest.json` bytes（包括空白和换行）。
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    /// 原始 manifest bytes 的 canonical `sha256:<64 lowercase hex>`。
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// 按 manifest path（也是 UTF-8 byte lexicographic 顺序）返回 payload。
    pub fn payloads(&self) -> impl ExactSizeIterator<Item = &VerifiedWebArtifactPayload> {
        self.payloads.iter()
    }

    /// 按相对 path 查找冻结 payload。
    pub fn payload(&self, path: &str) -> Option<&VerifiedWebArtifactPayload> {
        self.payloads
            .binary_search_by(|payload| payload.path().cmp(path))
            .ok()
            .map(|index| &self.payloads[index])
    }
}

/// 从绝对 artifact root 读取并冻结当前 Web artifact。
///
/// `expected_server_version` 必须是 host 当前发布的 numeric `major.minor.patch` 版本；
/// manifest 内的 `protocolVersion` 与其它 value contract 由 `kanban-protocol` 严格校验。
pub fn verify_directory(
    root: &Path,
    expected_server_version: &str,
) -> Result<VerifiedWebArtifact, WebArtifactVerificationError> {
    ensure_absolute_directory(root)?;

    let mut files = BTreeMap::new();
    let mut manifest_bytes = None;
    walk_directory(root, Path::new(""), &mut manifest_bytes, &mut files)?;

    let manifest_bytes = manifest_bytes.ok_or_else(|| {
        WebArtifactVerificationError::invalid(format!(
            "Web artifact 缺少 root {WEB_ARTIFACT_MANIFEST_PATH}"
        ))
    })?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|error| {
        WebArtifactVerificationError::invalid(format!(
            "Web artifact manifest 必须是 UTF-8: {error}"
        ))
    })?;
    let manifest = manifest_text
        .parse::<WebArtifactManifest>()
        .map_err(|error| WebArtifactVerificationError::invalid(error.to_string()))?;
    if manifest.server_version != expected_server_version {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact serverVersion 不匹配: manifest={}, expected={expected_server_version}",
            manifest.server_version
        )));
    }

    let expected_paths = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let actual_paths = files.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if let Some(path) = expected_paths.difference(&actual_paths).next() {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact 缺少 manifest 声明的 payload: {path}"
        )));
    }
    if let Some(path) = actual_paths.difference(&expected_paths).next() {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact 存在 manifest 未声明的 extra payload: {path}"
        )));
    }

    let mut payloads = Vec::with_capacity(manifest.files.len());
    for expected in &manifest.files {
        let bytes = files
            .get(&expected.path)
            .expect("exact path comparison above guarantees payload presence");
        let observed = web_artifact_file_from_bytes(&expected.path, bytes).map_err(|error| {
            WebArtifactVerificationError::invalid(format!(
                "Web artifact payload {} 摘要计算失败: {error}",
                expected.path
            ))
        })?;
        if &observed != expected {
            return Err(WebArtifactVerificationError::invalid(format!(
                "Web artifact payload {} bytes/hash mismatch: expected={expected:?}, observed={observed:?}",
                expected.path
            )));
        }
        payloads.push(VerifiedWebArtifactPayload {
            descriptor: expected.clone(),
            bytes: Arc::clone(bytes),
        });
    }

    Ok(VerifiedWebArtifact {
        manifest,
        manifest_sha256: web_artifact_sha256_for_bytes(&manifest_bytes),
        manifest_bytes,
        payloads,
    })
}

fn ensure_absolute_directory(root: &Path) -> Result<(), WebArtifactVerificationError> {
    if !root.is_absolute() {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact root 必须是 absolute path: {}",
            root.display()
        )));
    }

    // 逐级检查目录 component 且不跟随 component symlink；因此 `/tmp/link/dist` 会在
    // `read_dir` 遍历 linked parent 前失败。
    if root
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact root 不得包含 `.` 或 `..` path component: {}",
            root.display()
        )));
    }
    let mut current = PathBuf::new();
    for component in root.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                current.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => current.push(component.as_os_str()),
        }
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| WebArtifactVerificationError::io(&current, error))?;
        reject_symlink(&current, &metadata)?;
    }

    let metadata = fs::symlink_metadata(root)
        .map_err(|error| WebArtifactVerificationError::io(root, error))?;
    reject_symlink(root, &metadata)?;
    if !metadata.is_dir() {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact root 必须是 directory: {}",
            root.display()
        )));
    }
    Ok(())
}

fn walk_directory(
    root: &Path,
    relative: &Path,
    manifest_bytes: &mut Option<Arc<[u8]>>,
    files: &mut BTreeMap<String, Arc<[u8]>>,
) -> Result<(), WebArtifactVerificationError> {
    let directory = root.join(relative);
    let entries = fs::read_dir(&directory)
        .map_err(|error| WebArtifactVerificationError::io(&directory, error))?;
    let mut has_entry = false;

    for entry in entries {
        let entry = entry.map_err(|error| WebArtifactVerificationError::io(&directory, error))?;
        has_entry = true;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            WebArtifactVerificationError::invalid(format!(
                "Web artifact path 必须是 UTF-8: {}",
                entry.path().display()
            ))
        })?;
        let child_relative = if relative.as_os_str().is_empty() {
            PathBuf::from(name)
        } else {
            relative.join(name)
        };
        let child_path = entry.path();
        let metadata = fs::symlink_metadata(&child_path)
            .map_err(|error| WebArtifactVerificationError::io(&child_path, error))?;
        reject_symlink(&child_path, &metadata)?;

        if metadata.is_dir() {
            validate_relative_path(&child_relative)?;
            walk_directory(root, &child_relative, manifest_bytes, files)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(WebArtifactVerificationError::invalid(format!(
                "Web artifact path 不是 regular file/directory: {}",
                child_relative.display()
            )));
        }

        let path = child_relative.to_str().ok_or_else(|| {
            WebArtifactVerificationError::invalid(format!(
                "Web artifact path 必须是 UTF-8: {}",
                child_relative.display()
            ))
        })?;
        if path == WEB_ARTIFACT_MANIFEST_PATH && relative.as_os_str().is_empty() {
            if manifest_bytes.is_some() {
                return Err(WebArtifactVerificationError::invalid(
                    "Web artifact root manifest.json 重复",
                ));
            }
            *manifest_bytes = Some(read_stable_file(&child_path)?);
            continue;
        }

        validate_relative_path(&child_relative)?;
        let bytes = read_stable_file(&child_path)?;
        files.insert(path.to_owned(), bytes);
    }

    if !relative.as_os_str().is_empty() && !has_entry {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact 存在 empty directory: {}",
            relative.display()
        )));
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<(), WebArtifactVerificationError> {
    let value = path.to_str().ok_or_else(|| {
        WebArtifactVerificationError::invalid(format!(
            "Web artifact path 必须是 UTF-8: {}",
            path.display()
        ))
    })?;
    web_artifact_file_from_bytes(value, &[])
        .map(|_| ())
        .map_err(|error| WebArtifactVerificationError::invalid(error.to_string()))
}

fn reject_symlink(path: &Path, metadata: &Metadata) -> Result<(), WebArtifactVerificationError> {
    if metadata.file_type().is_symlink() {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact path 禁止 symlink: {}",
            path.display()
        )));
    }
    Ok(())
}

fn read_stable_file(path: &Path) -> Result<Arc<[u8]>, WebArtifactVerificationError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| WebArtifactVerificationError::io(path, error))?;
    ensure_regular_file(path, &before)?;
    let file = open_no_follow(path)?;
    let descriptor_before = file
        .metadata()
        .map_err(|error| WebArtifactVerificationError::io(path, error))?;
    ensure_regular_file(path, &descriptor_before)?;
    compare_metadata(path, "open", &before, &descriptor_before)?;

    let mut bytes = Vec::new();
    (&file)
        .take(u64::MAX)
        .read_to_end(&mut bytes)
        .map_err(|error| WebArtifactVerificationError::io(path, error))?;

    let descriptor_after = file
        .metadata()
        .map_err(|error| WebArtifactVerificationError::io(path, error))?;
    ensure_regular_file(path, &descriptor_after)?;
    compare_metadata(path, "read", &descriptor_before, &descriptor_after)?;
    if descriptor_after.len() != bytes.len() as u64 {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact path read length drift: {}",
            path.display()
        )));
    }
    let after = fs::symlink_metadata(path)
        .map_err(|error| WebArtifactVerificationError::io(path, error))?;
    ensure_regular_file(path, &after)?;
    compare_metadata(path, "path", &descriptor_after, &after)?;

    Ok(Arc::from(bytes.into_boxed_slice()))
}

fn ensure_regular_file(
    path: &Path,
    metadata: &Metadata,
) -> Result<(), WebArtifactVerificationError> {
    reject_symlink(path, metadata)?;
    if !metadata.is_file() {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact path 不是 regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact path 禁止 hardlink (nlink={}): {}",
            metadata.nlink(),
            path.display()
        )));
    }
    Ok(())
}

fn compare_metadata(
    path: &Path,
    phase: &str,
    expected: &Metadata,
    actual: &Metadata,
) -> Result<(), WebArtifactVerificationError> {
    if metadata_identity(expected) != metadata_identity(actual) {
        return Err(WebArtifactVerificationError::invalid(format!(
            "Web artifact path {phase} identity drift: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_identity(metadata: &Metadata) -> (u64, u64, u64, u64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.nlink(),
        metadata.len(),
    )
}

#[cfg(not(unix))]
fn metadata_identity(metadata: &Metadata) -> (u64, u64, u64, u64) {
    (0, 0, 1, metadata.len())
}

fn open_no_follow(path: &Path) -> Result<File, WebArtifactVerificationError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    options
        .open(path)
        .map_err(|error| WebArtifactVerificationError::io(path, error))
}
