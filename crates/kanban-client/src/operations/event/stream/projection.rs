use kanban_protocol::{
    ListEventsQuery, ListEventsResponse, MAX_SAFE_EVENT_CURSOR,
    rpc::{
        query::{MAX_QUERY_RESULT_BYTES, QUERY_CHUNK_BYTES},
        v1,
    },
};
use prost::Message;
use sha2::{Digest, Sha256};

use super::invalid;
use crate::ClientError;

#[derive(Default)]
pub(super) struct Projection {
    cursor: Option<v1::QueryCursor>,
    bytes: Vec<u8>,
    staging: Option<Staging>,
}

struct Staging {
    begin: v1::QueryBegin,
    patch: Vec<u8>,
    next_chunk: u32,
}

impl Projection {
    pub fn cursor(&self) -> Option<&v1::QueryCursor> {
        self.cursor.as_ref()
    }

    pub fn begin(&mut self, begin: v1::QueryBegin) -> Result<(), ClientError> {
        if self.staging.is_some() {
            return Err(invalid("查询 snapshot/delta begin 重叠"));
        }
        let cursor = begin
            .cursor
            .as_ref()
            .ok_or_else(|| invalid("查询 begin 缺少 cursor"))?;
        if cursor.epoch.is_empty()
            || cursor.scope.is_empty()
            || cursor.epoch.len() > 128
            || cursor.scope.len() > 128
            || cursor.revision == 0
        {
            return Err(invalid("查询 cursor 无效"));
        }
        let result_size = bounded(begin.result_size)?;
        let patch_size = bounded(begin.patch_size)?;
        if begin.result_sha256.len() != 32
            || usize::try_from(begin.chunk_count).ok()
                != Some(patch_size.div_ceil(QUERY_CHUNK_BYTES))
        {
            return Err(invalid("查询摘要或分块数量无效"));
        }
        if begin.snapshot {
            if begin.base.is_some()
                || begin.offset != 0
                || begin.delete_length != 0
                || patch_size != result_size
            {
                return Err(invalid("完整快照含有 delta 基线或无效大小"));
            }
            if self.cursor.as_ref().is_some_and(|previous| {
                previous.epoch == cursor.epoch
                    && (previous.scope != cursor.scope || previous.revision > cursor.revision)
            }) {
                return Err(invalid("同一查询收到倒退或错作用域快照"));
            }
        } else {
            let base = begin
                .base
                .as_ref()
                .ok_or_else(|| invalid("查询 delta 缺少基线"))?;
            if Some(base) != self.cursor.as_ref()
                || cursor.epoch != base.epoch
                || cursor.scope != base.scope
                || cursor.revision <= base.revision
            {
                return Err(invalid("查询 delta 与最后完整投影不连续"));
            }
            let offset = bounded(begin.offset)?;
            let delete = bounded(begin.delete_length)?;
            if offset > self.bytes.len()
                || delete > self.bytes.len() - offset
                || self.bytes.len() - delete + patch_size != result_size
            {
                return Err(invalid("查询 delta splice 范围无效"));
            }
        }
        self.staging = Some(Staging {
            begin,
            patch: Vec::with_capacity(patch_size),
            next_chunk: 0,
        });
        Ok(())
    }

    pub fn chunk(&mut self, chunk: v1::QueryChunk) -> Result<(), ClientError> {
        let staging = self
            .staging
            .as_mut()
            .ok_or_else(|| invalid("查询 chunk 出现在 begin 之前"))?;
        let total = bounded(staging.begin.patch_size)?;
        let expected_size = (total - staging.patch.len()).min(QUERY_CHUNK_BYTES);
        if chunk.index != staging.next_chunk
            || chunk.index >= staging.begin.chunk_count
            || chunk.data.len() != expected_size
        {
            return Err(invalid("查询 chunk 索引、大小或顺序无效"));
        }
        staging.patch.extend_from_slice(&chunk.data);
        staging.next_chunk += 1;
        Ok(())
    }

    pub fn end(
        &mut self,
        end: v1::QueryEnd,
        query: &ListEventsQuery,
    ) -> Result<ListEventsResponse, ClientError> {
        let staging = self
            .staging
            .take()
            .ok_or_else(|| invalid("查询 end 出现在 begin 之前"))?;
        if end.cursor != staging.begin.cursor
            || staging.patch.len() != bounded(staging.begin.patch_size)?
            || staging.next_chunk != staging.begin.chunk_count
        {
            return Err(invalid("查询 end 不匹配或分块不完整"));
        }
        let bytes = if staging.begin.snapshot {
            staging.patch
        } else {
            let offset = bounded(staging.begin.offset)?;
            let delete = bounded(staging.begin.delete_length)?;
            let mut bytes = Vec::with_capacity(bounded(staging.begin.result_size)?);
            bytes.extend_from_slice(&self.bytes[..offset]);
            bytes.extend_from_slice(&staging.patch);
            bytes.extend_from_slice(&self.bytes[offset + delete..]);
            bytes
        };
        if bytes.len() != bounded(staging.begin.result_size)?
            || Sha256::digest(&bytes).as_slice() != staging.begin.result_sha256
        {
            return Err(invalid("查询投影大小或 SHA-256 不一致"));
        }
        let result = v1::QueryResult::decode(bytes.as_slice())
            .map_err(|_| invalid("查询投影 Protobuf 损坏"))?;
        let Some(v1::query_result::Result::ListEvents(response)) = result.result else {
            return Err(invalid("事件流收到其他类型投影"));
        };
        let response: ListEventsResponse =
            response.try_into().map_err(ClientError::response_codec)?;
        validate_page(&response, query)?;
        // 类型、作用域、cursor 与完整字节校验都成功后才提交。
        self.bytes = bytes;
        self.cursor = staging.begin.cursor;
        Ok(response)
    }
}

fn bounded(value: u64) -> Result<usize, ClientError> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value <= MAX_QUERY_RESULT_BYTES)
        .ok_or_else(|| invalid("查询投影超过编码字节预算"))
}

fn validate_page(
    response: &ListEventsResponse,
    query: &ListEventsQuery,
) -> Result<(), ClientError> {
    let mut previous = query.after;
    for event in &response.data {
        if event.id <= previous
            || event.id > MAX_SAFE_EVENT_CURSOR
            || event.board_id != query.board
            || query
                .task_id
                .as_ref()
                .is_some_and(|task| event.task_id.as_ref() != Some(task))
        {
            return Err(invalid("事件投影的 cursor 或作用域无效"));
        }
        previous = event.id;
    }
    if response.meta.next_after != previous || response.data.len() > query.limit {
        return Err(invalid("事件投影的 next_after 或大小无效"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
