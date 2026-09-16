//! File capability: immutable content plus ordinary object relations. No second database or host.
pub(crate) mod migration;
mod model;
mod path;
pub(crate) mod store;
#[cfg(test)]
mod tests;
pub(crate) use model::StoredFile;
pub use model::{FileDownload, FileInfo, FilePage, FileUploadSpec};

use super::{ObjectError, ObjectResult};
use crate::{AttachmentUpload, KanbanService};
use kanban_core::Clock;

impl<C: Clock> KanbanService<C> {
    pub async fn file_scope(&self, owner_id: &str) -> ObjectResult<String> {
        let c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        store::scope(&c, owner_id).await
    }
    pub async fn file_page(
        &self,
        board_id: &str,
        owner_id: &str,
        limit: u32,
        after: &str,
    ) -> ObjectResult<FilePage> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = store::page(&tx, board_id, owner_id, limit, after).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn file_list(&self, board_id: &str, owner_id: &str) -> ObjectResult<Vec<FileInfo>> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = store::list(&tx, board_id, owner_id)
            .await?
            .into_iter()
            .map(|file| file.info)
            .collect();
        tx.commit().await?;
        Ok(result)
    }
    pub async fn file_upload_replay(
        &self,
        spec: &FileUploadSpec,
    ) -> ObjectResult<Option<FileInfo>> {
        store::validate_spec(spec)?;
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        store::owner(&tx, &spec.board_id, &spec.owner_id, false).await?;
        let result = store::replay(&tx, spec).await?;
        tx.commit().await?;
        if let Some(record) = result {
            let root = self
                .attachment_root
                .as_deref()
                .ok_or_else(|| ObjectError::precondition("file.root_unconfigured"))?;
            let _verified = store::open(root, &record).await?;
            Ok(Some(record.info))
        } else {
            Ok(None)
        }
    }
    pub async fn file_begin_upload(&self, spec: &FileUploadSpec) -> ObjectResult<AttachmentUpload> {
        store::validate_spec(spec)?;
        let c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        store::owner(&c, &spec.board_id, &spec.owner_id, true).await?;
        let root = self
            .attachment_root
            .as_deref()
            .ok_or_else(|| ObjectError::precondition("file.root_unconfigured"))?
            .to_owned();
        tokio::task::spawn_blocking(move || {
            AttachmentUpload::create(&root).map_err(ObjectError::storage)
        })
        .await
        .map_err(ObjectError::storage)?
    }
    pub async fn file_commit_upload(
        &self,
        spec: &FileUploadSpec,
        upload: AttachmentUpload,
    ) -> ObjectResult<FileInfo> {
        let _gate = self.mutation_gate.lock().await;
        let root = self
            .attachment_root
            .as_deref()
            .ok_or_else(|| ObjectError::precondition("file.root_unconfigured"))?;
        let record = store::commit(
            &self.store,
            root,
            spec,
            upload,
            &kanban_core::new_event_id(),
            self.clock.now_ms(),
        )
        .await?;
        Ok(record.info)
    }
    pub async fn file_download(
        &self,
        board_id: &str,
        owner_id: &str,
        id: &str,
    ) -> ObjectResult<FileDownload> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let record = store::lookup(&tx, board_id, owner_id, id).await?;
        tx.commit().await?;
        let root = self
            .attachment_root
            .as_deref()
            .ok_or_else(|| ObjectError::precondition("file.root_unconfigured"))?;
        let file = store::open(root, &record).await?;
        Ok(FileDownload {
            info: record.info,
            file,
        })
    }
}
