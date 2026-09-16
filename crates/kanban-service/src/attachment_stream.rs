//! 附件内容的有界暂存与不可覆盖发布。路径只由 service 生成。
use crate::StoreError;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicUsize, Ordering},
    },
};

pub const MAX_ATTACHMENT_BYTES: u64 = 256 * 1024 * 1024;
pub const ATTACHMENT_IO_CHUNK_BYTES: usize = 64 * 1024;
const MAX_ACTIVE_UPLOADS: usize = 4;
static ACTIVE_UPLOADS: OnceLock<Mutex<HashMap<PathBuf, Weak<AtomicUsize>>>> = OnceLock::new();

#[derive(Debug)]
struct UploadPermit(Arc<AtomicUsize>);
impl UploadPermit {
    fn acquire(root: &Path) -> Result<Self, StoreError> {
        let mut roots = ACTIVE_UPLOADS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| StoreError::AttachmentIo("附件并发计数锁失效".into()))?;
        roots.retain(|_, count| count.strong_count() > 0);
        let count = roots.get(root).and_then(Weak::upgrade).unwrap_or_else(|| {
            let count = Arc::new(AtomicUsize::new(0));
            roots.insert(root.to_path_buf(), Arc::downgrade(&count));
            count
        });
        count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                (value < MAX_ACTIVE_UPLOADS).then_some(value + 1)
            })
            .map_err(|_| StoreError::InvalidInput("附件上传繁忙，最多同时接收 4 个文件".into()))?;
        Ok(Self(count))
    }
}
impl Drop for UploadPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 持有 host 内部暂存文件。丢弃请求时清除暂存；进程崩溃残留由维护操作处理。
/// 不提供从客户端路径构造此类型的接口。
#[derive(Debug)]
pub struct AttachmentUpload {
    file: Option<File>,
    path: PathBuf,
    root: PathBuf,
    length: u64,
    hasher: Sha256,
    sealed: bool,
    _permit: UploadPermit,
}
impl AttachmentUpload {
    pub(crate) fn create(root: &Path) -> Result<Self, StoreError> {
        reject_symlink(root)?;
        let root = fs::canonicalize(root).map_err(io_error)?;
        let permit = UploadPermit::acquire(&root)?;
        let incoming = root.join(".incoming");
        match fs::create_dir(&incoming) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(error)),
        }
        reject_symlink(&incoming)?;
        if !fs::canonicalize(&incoming)
            .map_err(io_error)?
            .starts_with(&root)
        {
            return Err(StoreError::AttachmentIntegrity("附件暂存目录越界".into()));
        }
        let path = incoming.join(format!("{}.part", kanban_core::new_typed_id("upload")));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true).read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(io_error)?;
        Ok(Self {
            file: Some(file),
            path,
            root,
            length: 0,
            hasher: Sha256::new(),
            sealed: false,
            _permit: permit,
        })
    }

    /// 此同步 I/O 方法必须在 blocking worker 上调用。
    pub fn write_chunk(mut self, bytes: &[u8]) -> crate::Result<Self> {
        if self.sealed || bytes.len() > ATTACHMENT_IO_CHUNK_BYTES {
            return Err(crate::KanbanError::InvalidInput(
                "暂存已封口，或分块超过 64 KiB".into(),
            ));
        }
        let length = self
            .length
            .checked_add(bytes.len() as u64)
            .filter(|length| *length <= MAX_ATTACHMENT_BYTES)
            .ok_or_else(|| crate::KanbanError::InvalidInput("附件超过 256 MiB 上限".into()))?;
        self.file
            .as_mut()
            .ok_or_else(|| crate::KanbanError::Storage("暂存文件已关闭".into()))?
            .write_all(bytes)
            .map_err(|error| crate::KanbanError::Storage(error.to_string()))?;
        self.hasher.update(bytes);
        self.length = length;
        Ok(self)
    }

    /// 文件 fsync 成功后才能进入 metadata 事务。
    pub fn seal(mut self) -> crate::Result<Self> {
        if let Some(file) = self.file.take() {
            file.sync_all()
                .map_err(|error| crate::KanbanError::Storage(error.to_string()))?;
            // Windows 也在建立 hard link 前关闭写句柄。
            drop(file);
        }
        self.sealed = true;
        Ok(self)
    }

    /// A duplicate acknowledged chunk is accepted only when its exact bytes match.
    /// This runs on a blocking worker while the host holds this upload session's mutex.
    pub fn matches_chunk(&mut self, offset: u64, bytes: &[u8]) -> crate::Result<bool> {
        if bytes.len() > ATTACHMENT_IO_CHUNK_BYTES
            || offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > self.length)
        {
            return Ok(false);
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| crate::KanbanError::InvalidInput("file.upload_sealed".into()))?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| crate::KanbanError::Storage(error.to_string()))?;
        let mut old = vec![0; bytes.len()];
        let result = file.read_exact(&mut old);
        let restore = file.seek(SeekFrom::Start(self.length));
        result.map_err(|error| crate::KanbanError::Storage(error.to_string()))?;
        restore.map_err(|error| crate::KanbanError::Storage(error.to_string()))?;
        Ok(old == bytes)
    }

    pub fn size_bytes(&self) -> u64 {
        self.length
    }
    pub(crate) fn sha256(&self) -> String {
        format!("{:x}", self.hasher.clone().finalize())
    }

    /// 返回是否新建目标。崩溃后同 ID 重试可接管内容完全相同的孤立文件。
    pub(crate) fn publish(&self, target: &Path) -> Result<bool, StoreError> {
        if !self.sealed {
            return Err(StoreError::AttachmentIntegrity("暂存文件尚未封口".into()));
        }
        let parent = target
            .parent()
            .ok_or_else(|| StoreError::AttachmentIntegrity("附件路径缺少父目录".into()))?;
        if !fs::canonicalize(parent)
            .map_err(io_error)?
            .starts_with(&self.root)
        {
            return Err(StoreError::AttachmentIntegrity("附件发布路径越界".into()));
        }
        match fs::hard_link(&self.path, target) {
            Ok(()) => {
                // 失败时保留目标，让后续同 ID 重试验证并恢复；不能不加区分地删文件。
                sync_directory(parent)?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = open_verified(target, self.length as i64, Some(&self.sha256()))?;
                Ok(false)
            }
            Err(error) => Err(io_error(error)),
        }
    }
}
impl Drop for AttachmentUpload {
    fn drop(&mut self) {
        drop(self.file.take());
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(path = %self.path.display(), %error, "附件暂存清理失败，需要维护检查");
        }
    }
}

pub(crate) fn open_verified(
    path: &Path,
    expected_size: i64,
    expected_sha: Option<&str>,
) -> Result<File, StoreError> {
    if !(0..=MAX_ATTACHMENT_BYTES as i64).contains(&expected_size) {
        return Err(StoreError::AttachmentIntegrity(
            "附件元数据大小超出允许范围".into(),
        ));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(StoreError::AttachmentIntegrity(
                "附件路径不能是符号链接".into(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(StoreError::AttachmentFileMissing(
                path.display().to_string(),
            ));
        }
        Err(error) => return Err(io_error(error)),
    }
    let mut file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StoreError::AttachmentFileMissing(path.display().to_string())
        } else {
            io_error(error)
        }
    })?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() || metadata.len() != expected_size as u64 {
        return Err(StoreError::AttachmentIntegrity(
            "附件不是普通文件，或大小校验失败".into(),
        ));
    }
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; ATTACHMENT_IO_CHUNK_BYTES];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > expected_size as u64 {
            return Err(StoreError::AttachmentIntegrity(
                "校验期间附件大小发生变化".into(),
            ));
        }
        hasher.update(&buffer[..count]);
    }
    if total != expected_size as u64
        || expected_sha.is_some_and(|sha| sha != format!("{:x}", hasher.finalize()))
    {
        return Err(StoreError::AttachmentIntegrity(
            "附件 SHA-256 或大小校验失败".into(),
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    Ok(file)
}
fn reject_symlink(path: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() {
        return Err(StoreError::AttachmentIntegrity(
            "附件路径不能是符号链接".into(),
        ));
    }
    Ok(())
}
fn io_error(error: std::io::Error) -> StoreError {
    StoreError::AttachmentIo(error.to_string())
}
fn sync_directory(path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(io_error)?;
    }
    // std 的 Windows 目录句柄 fsync 不能提供同等保证；见安全与恢复文档。
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_file_is_valid_and_stage_is_cleaned() {
        let root = tempfile::tempdir().unwrap();
        let upload = AttachmentUpload::create(root.path())
            .unwrap()
            .seal()
            .unwrap();
        let stage = upload.path.clone();
        let target = root.path().join("empty.blob");
        assert!(upload.publish(&target).unwrap());
        assert!(!upload.publish(&target).unwrap());
        drop(upload);
        assert!(!stage.exists());
        assert_eq!(fs::read(target).unwrap(), b"");
    }
    #[test]
    fn changed_content_cannot_replace_existing_target() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("file.blob");
        fs::write(&target, b"old").unwrap();
        let upload = AttachmentUpload::create(root.path())
            .unwrap()
            .write_chunk(b"new")
            .unwrap()
            .seal()
            .unwrap();
        assert!(upload.publish(&target).is_err());
        assert_eq!(fs::read(target).unwrap(), b"old");
    }
    #[test]
    fn chunk_and_total_limits_are_enforced_before_write() {
        let root = tempfile::tempdir().unwrap();
        let upload = AttachmentUpload::create(root.path()).unwrap();
        assert!(
            upload
                .write_chunk(&vec![0; ATTACHMENT_IO_CHUNK_BYTES + 1])
                .is_err()
        );
        let mut upload = AttachmentUpload::create(root.path()).unwrap();
        upload.length = MAX_ATTACHMENT_BYTES;
        assert!(upload.write_chunk(b"x").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn incoming_symlink_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(other.path(), root.path().join(".incoming")).unwrap();
        assert!(AttachmentUpload::create(root.path()).is_err());
    }
}
