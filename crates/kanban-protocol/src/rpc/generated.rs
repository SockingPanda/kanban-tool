use super::codec::{decode_json, encode_json, finite, required, validate_stream_event};
use super::{RpcCodecError, v1};
impl v1::GetHealthRequest {
    pub fn decode_parts(self) -> Result<((), (), ()), RpcCodecError> {
        Ok(((), (), ()))
    }
    pub fn from_parts(_path: (), _query: (), _input: ()) -> Result<Self, RpcCodecError> {
        Ok(Self {})
    }
}
impl v1::ListBoardsRequest {
    pub fn decode_parts(self) -> Result<((), crate::boards::ListBoardsQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::boards::ListBoardsQuery {
                include_archived: self.include_archived.unwrap_or_default(),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::boards::ListBoardsQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            include_archived: Some(query.include_archived),
        })
    }
}
impl v1::CreateBoardRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::boards::CreateBoardRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::boards::CreateBoardRequest {
                slug: required(self.slug, "slug")?,
                name: required(self.name, "name")?,
                description: self.description,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::boards::CreateBoardRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            slug: Some(input.slug),
            name: Some(input.name),
            description: input.description,
            actor: input.actor,
        })
    }
}
impl v1::GetBoardRequest {
    pub fn decode_parts(self) -> Result<(crate::boards::GetBoardPath, (), ()), RpcCodecError> {
        Ok((
            crate::boards::GetBoardPath {
                board: required(self.board, "board")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::boards::GetBoardPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
        })
    }
}
impl v1::ArchiveBoardRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::boards::ArchiveBoardPath,
            (),
            crate::lifecycle::ArchiveBoardRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::boards::ArchiveBoardPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::lifecycle::ArchiveBoardRequest { actor: self.actor },
        ))
    }
    pub fn from_parts(
        path: crate::boards::ArchiveBoardPath,
        _query: (),
        input: crate::lifecycle::ArchiveBoardRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            actor: input.actor,
        })
    }
}
impl v1::ListBoardColumnsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::boards::ListBoardColumnsPath, (), ()), RpcCodecError> {
        Ok((
            crate::boards::ListBoardColumnsPath {
                board: required(self.board, "board")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::boards::ListBoardColumnsPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
        })
    }
}
impl v1::ListTasksRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::task_read::ListTasksPath,
            crate::task_read::ListTasksQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::task_read::ListTasksPath {
                board: required(self.board, "board")?,
            },
            crate::task_read::ListTasksQuery {
                status: (self.status)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoApiTaskStatus::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                priority: (self.priority)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .collect::<Result<Vec<_>, _>>()?,
                label: (self.label)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .collect::<Result<Vec<_>, _>>()?,
                plan_filter: (self.plan_filter)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoTaskReadPlanFilter::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                assignee: self.assignee,
                q: self.q,
                include_archived: self.include_archived.unwrap_or_else(|| {
                    <crate::task_read::ListTasksQuery>::default().include_archived
                }),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_read::ListTasksQuery>::default().limit,
                },
                offset: match self.offset {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_read::ListTasksQuery>::default().offset,
                },
                sort: match self.sort {
                    Some(value) => v1::DtoTaskReadSort::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                        .try_into()?,
                    None => <crate::task_read::ListTasksQuery>::default().sort,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::task_read::ListTasksPath,
        query: crate::task_read::ListTasksQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            status: (query.status)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoApiTaskStatus::try_from(value)?))
                })
                .collect::<Result<Vec<_>, _>>()?,
            priority: (query.priority)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            label: (query.label)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            plan_filter: (query.plan_filter)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoTaskReadPlanFilter::try_from(value)?))
                })
                .collect::<Result<Vec<_>, _>>()?,
            assignee: query.assignee,
            q: query.q,
            include_archived: Some(query.include_archived),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            offset: Some(
                u64::try_from(query.offset)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            sort: Some(i32::from(v1::DtoTaskReadSort::try_from(query.sort)?)),
        })
    }
}
impl v1::ListTasksByStatusRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::task_read::ListTasksByStatusPath,
            crate::task_read::ListTasksByStatusQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::task_read::ListTasksByStatusPath {
                board: required(self.board, "board")?,
            },
            crate::task_read::ListTasksByStatusQuery {
                status: (self.status)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoApiTaskStatus::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                priority: (self.priority)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .collect::<Result<Vec<_>, _>>()?,
                label: (self.label)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .collect::<Result<Vec<_>, _>>()?,
                plan_filter: (self.plan_filter)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoTaskReadPlanFilter::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                assignee: self.assignee,
                q: self.q,
                include_archived: self.include_archived.unwrap_or_else(|| {
                    <crate::task_read::ListTasksByStatusQuery>::default().include_archived
                }),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_read::ListTasksByStatusQuery>::default().limit,
                },
                offset: match self.offset {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_read::ListTasksByStatusQuery>::default().offset,
                },
                sort: match self.sort {
                    Some(value) => v1::DtoTaskReadSort::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                        .try_into()?,
                    None => <crate::task_read::ListTasksByStatusQuery>::default().sort,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::task_read::ListTasksByStatusPath,
        query: crate::task_read::ListTasksByStatusQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            status: (query.status)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoApiTaskStatus::try_from(value)?))
                })
                .collect::<Result<Vec<_>, _>>()?,
            priority: (query.priority)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            label: (query.label)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            plan_filter: (query.plan_filter)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoTaskReadPlanFilter::try_from(value)?))
                })
                .collect::<Result<Vec<_>, _>>()?,
            assignee: query.assignee,
            q: query.q,
            include_archived: Some(query.include_archived),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            offset: Some(
                u64::try_from(query.offset)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            sort: Some(i32::from(v1::DtoTaskReadSort::try_from(query.sort)?)),
        })
    }
}
impl v1::CreateTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::create_task::CreateTaskPath,
            (),
            crate::create_task::CreateTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::create_task::CreateTaskPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::create_task::CreateTaskRequest {
                task_id: self.task_id,
                idempotency_key: self.idempotency_key,
                title: required(self.title, "title")?,
                description: self.description,
                status: self
                    .status
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoApiCreateTaskStatus::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .transpose()?,
                assignee: self.assignee,
                priority: self.priority.unwrap_or(3),
                scheduled_at: self.scheduled_at,
                due_at: self.due_at,
                max_retries: self.max_retries,
                metadata: self
                    .metadata
                    .map(|value| -> Result<_, RpcCodecError> {
                        ((value).entries)
                            .into_iter()
                            .map(|(key, value)| -> Result<_, RpcCodecError> {
                                Ok((key, decode_json(value)?))
                            })
                            .collect::<Result<_, _>>()
                    })
                    .transpose()?,
                labels: self.labels,
                depends_on: self.depends_on,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::create_task::CreateTaskPath,
        _query: (),
        input: crate::create_task::CreateTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            task_id: input.task_id,
            idempotency_key: input.idempotency_key,
            title: Some(input.title),
            description: input.description,
            status: input
                .status
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoApiCreateTaskStatus::try_from(value)?))
                })
                .transpose()?,
            assignee: input.assignee,
            priority: Some(input.priority),
            scheduled_at: input.scheduled_at,
            due_at: input.due_at,
            max_retries: input.max_retries,
            metadata: input
                .metadata
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(v1::MapOfJsonValue {
                        entries: (value)
                            .into_iter()
                            .map(|(key, value)| -> Result<_, RpcCodecError> {
                                Ok((key, encode_json(value)?))
                            })
                            .collect::<Result<_, _>>()?,
                    })
                })
                .transpose()?,
            labels: input.labels,
            depends_on: input.depends_on,
            actor: input.actor,
        })
    }
}
impl v1::GetTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::task_core::GetTaskPath,
            crate::task_core::GetTaskQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::task_core::GetTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            crate::task_core::GetTaskQuery {
                include: self.include,
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::task_core::GetTaskPath,
        query: crate::task_core::GetTaskQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            include: query.include,
        })
    }
}
impl v1::UpdateTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::task_core::UpdateTaskPath,
            (),
            crate::task_core::UpdateTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::task_core::UpdateTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::task_core::UpdateTaskRequest {
                title: self.title,
                description: self
                    .description
                    .map(|patch| -> Result<_, RpcCodecError> {
                        Ok(match required(patch.change, "description")? {
                            v1::patch_string::Change::Clear(_) => None,
                            v1::patch_string::Change::Value(value) => Some(value),
                        })
                    })
                    .transpose()?,
                assignee: self
                    .assignee
                    .map(|patch| -> Result<_, RpcCodecError> {
                        Ok(match required(patch.change, "assignee")? {
                            v1::patch_string::Change::Clear(_) => None,
                            v1::patch_string::Change::Value(value) => Some(value),
                        })
                    })
                    .transpose()?,
                priority: self.priority,
                scheduled_at: self
                    .scheduled_at
                    .map(|patch| -> Result<_, RpcCodecError> {
                        Ok(match required(patch.change, "scheduled_at")? {
                            v1::patch_i64::Change::Clear(_) => None,
                            v1::patch_i64::Change::Value(value) => Some(value),
                        })
                    })
                    .transpose()?,
                due_at: self
                    .due_at
                    .map(|patch| -> Result<_, RpcCodecError> {
                        Ok(match required(patch.change, "due_at")? {
                            v1::patch_i64::Change::Clear(_) => None,
                            v1::patch_i64::Change::Value(value) => Some(value),
                        })
                    })
                    .transpose()?,
                max_retries: self
                    .max_retries
                    .map(|patch| -> Result<_, RpcCodecError> {
                        Ok(match required(patch.change, "max_retries")? {
                            v1::patch_i64::Change::Clear(_) => None,
                            v1::patch_i64::Change::Value(value) => Some(value),
                        })
                    })
                    .transpose()?,
                metadata: self
                    .metadata
                    .map(|patch| -> Result<_, RpcCodecError> {
                        Ok(match required(patch.change, "metadata")? {
                            v1::patch_json_value::Change::Clear(_) => None,
                            v1::patch_json_value::Change::Value(value) => Some(decode_json(value)?),
                        })
                    })
                    .transpose()?,
                actor: self.actor,
                expected_lock_version: self.expected_lock_version,
            },
        ))
    }
    pub fn from_parts(
        path: crate::task_core::UpdateTaskPath,
        _query: (),
        input: crate::task_core::UpdateTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            title: input.title,
            description: input
                .description
                .map(|change| -> Result<_, RpcCodecError> {
                    Ok(v1::PatchString {
                        change: Some(match change {
                            None => v1::patch_string::Change::Clear(v1::Empty {}),
                            Some(value) => v1::patch_string::Change::Value(value),
                        }),
                    })
                })
                .transpose()?,
            assignee: input
                .assignee
                .map(|change| -> Result<_, RpcCodecError> {
                    Ok(v1::PatchString {
                        change: Some(match change {
                            None => v1::patch_string::Change::Clear(v1::Empty {}),
                            Some(value) => v1::patch_string::Change::Value(value),
                        }),
                    })
                })
                .transpose()?,
            priority: input.priority,
            scheduled_at: input
                .scheduled_at
                .map(|change| -> Result<_, RpcCodecError> {
                    Ok(v1::PatchI64 {
                        change: Some(match change {
                            None => v1::patch_i64::Change::Clear(v1::Empty {}),
                            Some(value) => v1::patch_i64::Change::Value(value),
                        }),
                    })
                })
                .transpose()?,
            due_at: input
                .due_at
                .map(|change| -> Result<_, RpcCodecError> {
                    Ok(v1::PatchI64 {
                        change: Some(match change {
                            None => v1::patch_i64::Change::Clear(v1::Empty {}),
                            Some(value) => v1::patch_i64::Change::Value(value),
                        }),
                    })
                })
                .transpose()?,
            max_retries: input
                .max_retries
                .map(|change| -> Result<_, RpcCodecError> {
                    Ok(v1::PatchI64 {
                        change: Some(match change {
                            None => v1::patch_i64::Change::Clear(v1::Empty {}),
                            Some(value) => v1::patch_i64::Change::Value(value),
                        }),
                    })
                })
                .transpose()?,
            metadata: input
                .metadata
                .map(|change| -> Result<_, RpcCodecError> {
                    Ok(v1::PatchJsonValue {
                        change: Some(match change {
                            None => v1::patch_json_value::Change::Clear(v1::Empty {}),
                            Some(value) => v1::patch_json_value::Change::Value(encode_json(value)?),
                        }),
                    })
                })
                .transpose()?,
            actor: input.actor,
            expected_lock_version: input.expected_lock_version,
        })
    }
}
impl v1::SpecifyTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::SpecifyTaskPath,
            (),
            crate::lifecycle::SpecifyTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::SpecifyTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::SpecifyTaskRequest {
                actor: self.actor,
                description: self.description,
                scheduled_at: self.scheduled_at,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::SpecifyTaskPath,
        _query: (),
        input: crate::lifecycle::SpecifyTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            description: input.description,
            scheduled_at: input.scheduled_at,
        })
    }
}
impl v1::PromoteTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::PromoteTaskPath,
            (),
            crate::lifecycle::PromoteTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::PromoteTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::PromoteTaskRequest { actor: self.actor },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::PromoteTaskPath,
        _query: (),
        input: crate::lifecycle::PromoteTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
        })
    }
}
impl v1::ClaimTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::ClaimTaskPath,
            (),
            crate::lifecycle::ClaimTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::ClaimTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::ClaimTaskRequest {
                actor: self.actor,
                ttl_ms: self.ttl_ms.unwrap_or(300_000),
                worker_profile: self.worker_profile,
                metadata: self
                    .metadata
                    .map(|value| -> Result<_, RpcCodecError> { decode_json(value) })
                    .transpose()?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::ClaimTaskPath,
        _query: (),
        input: crate::lifecycle::ClaimTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            ttl_ms: Some(input.ttl_ms),
            worker_profile: input.worker_profile,
            metadata: input
                .metadata
                .map(|value| -> Result<_, RpcCodecError> { encode_json(value) })
                .transpose()?,
        })
    }
}
impl v1::ReopenTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::ReopenTaskPath,
            (),
            crate::lifecycle::ReopenTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::ReopenTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::ReopenTaskRequest {
                actor: self.actor,
                reason: required(self.reason, "reason")?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::ReopenTaskPath,
        _query: (),
        input: crate::lifecycle::ReopenTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            reason: Some(input.reason),
        })
    }
}
impl v1::ReclaimTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::ReclaimTaskPath,
            (),
            crate::lifecycle::ReclaimTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::ReclaimTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::ReclaimTaskRequest {
                actor: self.actor,
                force: self.force.unwrap_or_default(),
                to_status: self
                    .to_status
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoReclaimTargetStatus::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .transpose()?,
                reason: self.reason,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::ReclaimTaskPath,
        _query: (),
        input: crate::lifecycle::ReclaimTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            force: Some(input.force),
            to_status: input
                .to_status
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoReclaimTargetStatus::try_from(value)?))
                })
                .transpose()?,
            reason: input.reason,
        })
    }
}
impl v1::HeartbeatTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::HeartbeatTaskPath,
            (),
            crate::lifecycle::HeartbeatTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::HeartbeatTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::HeartbeatTaskRequest {
                actor: self.actor,
                claim_token: required(self.claim_token, "claim_token")?,
                ttl_ms: self.ttl_ms.unwrap_or(300_000),
                note: self.note,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::HeartbeatTaskPath,
        _query: (),
        input: crate::lifecycle::HeartbeatTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            claim_token: Some(input.claim_token),
            ttl_ms: Some(input.ttl_ms),
            note: input.note,
        })
    }
}
impl v1::ReleaseTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::ReleaseTaskPath,
            (),
            crate::lifecycle::ReleaseTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::ReleaseTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::ReleaseTaskRequest {
                actor: self.actor,
                claim_token: required(self.claim_token, "claim_token")?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::ReleaseTaskPath,
        _query: (),
        input: crate::lifecycle::ReleaseTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            claim_token: Some(input.claim_token),
        })
    }
}
impl v1::CompleteTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::CompleteTaskPath,
            (),
            crate::lifecycle::CompleteTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::CompleteTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::CompleteTaskRequest {
                actor: self.actor,
                claim_token: self.claim_token,
                force: self.force.unwrap_or_default(),
                summary: self.summary,
                result: self
                    .result
                    .map(|value| -> Result<_, RpcCodecError> { decode_json(value) })
                    .transpose()?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::CompleteTaskPath,
        _query: (),
        input: crate::lifecycle::CompleteTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            claim_token: input.claim_token,
            force: Some(input.force),
            summary: input.summary,
            result: input
                .result
                .map(|value| -> Result<_, RpcCodecError> { encode_json(value) })
                .transpose()?,
        })
    }
}
impl v1::SubmitReviewTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::SubmitReviewTaskPath,
            (),
            crate::lifecycle::SubmitReviewTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::SubmitReviewTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::SubmitReviewTaskRequest {
                actor: self.actor,
                claim_token: self.claim_token,
                force: self.force.unwrap_or_default(),
                summary: self.summary,
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::SubmitReviewTaskPath,
        _query: (),
        input: crate::lifecycle::SubmitReviewTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            claim_token: input.claim_token,
            force: Some(input.force),
            summary: input.summary,
        })
    }
}
impl v1::BlockTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::BlockTaskPath,
            (),
            crate::lifecycle::BlockTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::BlockTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::BlockTaskRequest {
                actor: self.actor,
                reason: required(self.reason, "reason")?,
                claim_token: self.claim_token,
                force: self.force.unwrap_or_default(),
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::BlockTaskPath,
        _query: (),
        input: crate::lifecycle::BlockTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            reason: Some(input.reason),
            claim_token: input.claim_token,
            force: Some(input.force),
        })
    }
}
impl v1::UnblockTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::UnblockTaskPath,
            (),
            crate::lifecycle::UnblockTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::UnblockTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::UnblockTaskRequest { actor: self.actor },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::UnblockTaskPath,
        _query: (),
        input: crate::lifecycle::UnblockTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
        })
    }
}
impl v1::ArchiveTaskRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::transitions::ArchiveTaskPath,
            (),
            crate::lifecycle::ArchiveTaskRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::transitions::ArchiveTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::ArchiveTaskRequest {
                actor: self.actor,
                force: self.force.unwrap_or_default(),
            },
        ))
    }
    pub fn from_parts(
        path: crate::transitions::ArchiveTaskPath,
        _query: (),
        input: crate::lifecycle::ArchiveTaskRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            actor: input.actor,
            force: Some(input.force),
        })
    }
}
impl v1::ListStepsRequest {
    pub fn decode_parts(self) -> Result<(crate::steps::ListStepsPath, (), ()), RpcCodecError> {
        Ok((
            crate::steps::ListStepsPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::steps::ListStepsPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
        })
    }
}
impl v1::CreateStepRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::steps::CreateStepPath,
            (),
            crate::steps::CreateStepRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::steps::CreateStepPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::steps::CreateStepRequest {
                idempotency_key: self.idempotency_key,
                title: required(self.title, "title")?,
                body: self.body,
                linked_task_ref: self.linked_task_ref,
                position: self.position,
                required: self.required.unwrap_or(true),
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::steps::CreateStepPath,
        _query: (),
        input: crate::steps::CreateStepRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            idempotency_key: input.idempotency_key,
            title: Some(input.title),
            body: input.body,
            linked_task_ref: input.linked_task_ref,
            position: input.position,
            required: Some(input.required),
            actor: input.actor,
        })
    }
}
impl v1::UpdateStepRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::steps::UpdateStepPath,
            (),
            crate::steps::UpdateStepRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::steps::UpdateStepPath {
                task_id: required(self.task_id, "task_id")?,
                step_id: required(self.step_id, "step_id")?,
            },
            (),
            crate::steps::UpdateStepRequest {
                title: self.title,
                body: self.body,
                linked_task_ref: self.linked_task_ref,
                unlink_task: self.unlink_task.unwrap_or_default(),
                position: self.position,
                required: self.required,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::steps::UpdateStepPath,
        _query: (),
        input: crate::steps::UpdateStepRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            step_id: Some(path.step_id),
            title: input.title,
            body: input.body,
            linked_task_ref: input.linked_task_ref,
            unlink_task: Some(input.unlink_task),
            position: input.position,
            required: input.required,
            actor: input.actor,
        })
    }
}
impl v1::RemoveStepRequest {
    pub fn decode_parts(self) -> Result<(crate::steps::RemoveStepPath, (), ()), RpcCodecError> {
        Ok((
            crate::steps::RemoveStepPath {
                task_id: required(self.task_id, "task_id")?,
                step_id: required(self.step_id, "step_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::steps::RemoveStepPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            step_id: Some(path.step_id),
        })
    }
}
impl v1::CompleteStepRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::steps::CompleteStepPath,
            (),
            crate::steps::CompleteStepRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::steps::CompleteStepPath {
                task_id: required(self.task_id, "task_id")?,
                step_id: required(self.step_id, "step_id")?,
            },
            (),
            crate::steps::CompleteStepRequest {
                note: required(self.note, "note")?,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::steps::CompleteStepPath,
        _query: (),
        input: crate::steps::CompleteStepRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            step_id: Some(path.step_id),
            note: Some(input.note),
            actor: input.actor,
        })
    }
}
impl v1::SkipStepRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::steps::SkipStepPath,
            (),
            crate::steps::SkipStepRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::steps::SkipStepPath {
                task_id: required(self.task_id, "task_id")?,
                step_id: required(self.step_id, "step_id")?,
            },
            (),
            crate::steps::SkipStepRequest {
                reason: required(self.reason, "reason")?,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::steps::SkipStepPath,
        _query: (),
        input: crate::steps::SkipStepRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            step_id: Some(path.step_id),
            reason: Some(input.reason),
            actor: input.actor,
        })
    }
}
impl v1::ReopenStepRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::steps::ReopenStepPath,
            (),
            crate::steps::ReopenStepRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::steps::ReopenStepPath {
                task_id: required(self.task_id, "task_id")?,
                step_id: required(self.step_id, "step_id")?,
            },
            (),
            crate::steps::ReopenStepRequest {
                reason: required(self.reason, "reason")?,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::steps::ReopenStepPath,
        _query: (),
        input: crate::steps::ReopenStepRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            step_id: Some(path.step_id),
            reason: Some(input.reason),
            actor: input.actor,
        })
    }
}
impl v1::MarkExecutionPlanNotRequiredRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::steps::MarkExecutionPlanNotRequiredPath,
            (),
            crate::steps::MarkExecutionPlanNotRequiredRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::steps::MarkExecutionPlanNotRequiredPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::steps::MarkExecutionPlanNotRequiredRequest {
                reason: required(self.reason, "reason")?,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::steps::MarkExecutionPlanNotRequiredPath,
        _query: (),
        input: crate::steps::MarkExecutionPlanNotRequiredRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            reason: Some(input.reason),
            actor: input.actor,
        })
    }
}
impl v1::ListDependenciesRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::dependencies::ListDependenciesPath, (), ()), RpcCodecError> {
        Ok((
            crate::dependencies::ListDependenciesPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::dependencies::ListDependenciesPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
        })
    }
}
impl v1::AddDependencyRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::dependencies::AddDependencyPath,
            (),
            crate::lifecycle::AddDependencyRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::dependencies::AddDependencyPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::lifecycle::AddDependencyRequest {
                parent_task_id: required(self.parent_task_id, "parent_task_id")?,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::dependencies::AddDependencyPath,
        _query: (),
        input: crate::lifecycle::AddDependencyRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            parent_task_id: Some(input.parent_task_id),
            actor: input.actor,
        })
    }
}
impl v1::RemoveDependencyRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::dependencies::RemoveDependencyPath, (), ()), RpcCodecError> {
        Ok((
            crate::dependencies::RemoveDependencyPath {
                child_task_id: required(self.child_task_id, "child_task_id")?,
                parent_task_id: required(self.parent_task_id, "parent_task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::dependencies::RemoveDependencyPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            child_task_id: Some(path.child_task_id),
            parent_task_id: Some(path.parent_task_id),
        })
    }
}
impl v1::ListRunsRequest {
    pub fn decode_parts(self) -> Result<(crate::runs::ListRunsPath, (), ()), RpcCodecError> {
        Ok((
            crate::runs::ListRunsPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::runs::ListRunsPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
        })
    }
}
impl v1::GetRunRequest {
    pub fn decode_parts(self) -> Result<(crate::runs::GetRunPath, (), ()), RpcCodecError> {
        Ok((
            crate::runs::GetRunPath {
                run_id: required(self.run_id, "run_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::runs::GetRunPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            run_id: Some(path.run_id),
        })
    }
}
impl v1::GetRunLogRequest {
    pub fn decode_parts(self) -> Result<(crate::runs::GetRunLogPath, (), ()), RpcCodecError> {
        Ok((
            crate::runs::GetRunLogPath {
                run_id: required(self.run_id, "run_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::runs::GetRunLogPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            run_id: Some(path.run_id),
        })
    }
}
impl v1::ListCommentsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::comments::ListCommentsPath, (), ()), RpcCodecError> {
        Ok((
            crate::comments::ListCommentsPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::comments::ListCommentsPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
        })
    }
}
impl v1::CreateCommentRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::comments::CreateCommentPath,
            (),
            crate::comments::CreateCommentRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::comments::CreateCommentPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::comments::CreateCommentRequest {
                idempotency_key: self.idempotency_key,
                author: self.author,
                body: required(self.body, "body")?,
                kind: self
                    .kind
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoCommentsCommentKind::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .transpose()?,
                author_type: self
                    .author_type
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoCommentsCommentAuthorType::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .transpose()?,
                agent_type: self.agent_type,
                metadata: self
                    .metadata
                    .map(|value| -> Result<_, RpcCodecError> { decode_json(value) })
                    .transpose()?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::comments::CreateCommentPath,
        _query: (),
        input: crate::comments::CreateCommentRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            idempotency_key: input.idempotency_key,
            author: input.author,
            body: Some(input.body),
            kind: input
                .kind
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoCommentsCommentKind::try_from(value)?))
                })
                .transpose()?,
            author_type: input
                .author_type
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoCommentsCommentAuthorType::try_from(
                        value,
                    )?))
                })
                .transpose()?,
            agent_type: input.agent_type,
            metadata: input
                .metadata
                .map(|value| -> Result<_, RpcCodecError> { encode_json(value) })
                .transpose()?,
        })
    }
}
impl v1::ListAttachmentsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::attachments::ListAttachmentsPath, (), ()), RpcCodecError> {
        Ok((
            crate::attachments::ListAttachmentsPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::attachments::ListAttachmentsPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
        })
    }
}
impl v1::CreateAttachmentRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::attachments::CreateAttachmentPath,
            (),
            crate::attachments::CreateAttachmentRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::attachments::CreateAttachmentPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::attachments::CreateAttachmentRequest {
                id: self.id,
                filename: required(self.filename, "filename")?,
                content: self.content,
                content_type: self.content_type,
                rel_path: self.rel_path,
                sha256: self.sha256,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::attachments::CreateAttachmentPath,
        _query: (),
        input: crate::attachments::CreateAttachmentRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            id: input.id,
            filename: Some(input.filename),
            content: input.content,
            content_type: input.content_type,
            rel_path: input.rel_path,
            sha256: input.sha256,
            actor: input.actor,
        })
    }
}
impl v1::DownloadAttachmentRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::attachments::GetAttachmentPath, (), ()), RpcCodecError> {
        Ok((
            crate::attachments::GetAttachmentPath {
                task_id: required(self.task_id, "task_id")?,
                attachment_id: required(self.attachment_id, "attachment_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::attachments::GetAttachmentPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            attachment_id: Some(path.attachment_id),
        })
    }
}
impl v1::DeleteAttachmentRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::attachments::GetAttachmentPath, (), ()), RpcCodecError> {
        Ok((
            crate::attachments::GetAttachmentPath {
                task_id: required(self.task_id, "task_id")?,
                attachment_id: required(self.attachment_id, "attachment_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::attachments::GetAttachmentPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            attachment_id: Some(path.attachment_id),
        })
    }
}
impl v1::ListEventsRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::ListEventsQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::ListEventsQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
                task_id: self.task_id,
                after: self.after.unwrap_or_default(),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 100,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::ListEventsQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
            task_id: query.task_id,
            after: Some(query.after),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::ListTaskLabelsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::labels::ListTaskLabelsPath, (), ()), RpcCodecError> {
        Ok((
            crate::labels::ListTaskLabelsPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::labels::ListTaskLabelsPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
        })
    }
}
impl v1::AddTaskLabelRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::labels::AddTaskLabelPath,
            (),
            crate::labels::AddTaskLabelRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::labels::AddTaskLabelPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::labels::AddTaskLabelRequest {
                name: self.name,
                names: self
                    .names
                    .map(|value| -> Result<_, RpcCodecError> { Ok((value).items) })
                    .transpose()?,
                create_missing: self.create_missing.unwrap_or_default(),
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::labels::AddTaskLabelPath,
        _query: (),
        input: crate::labels::AddTaskLabelRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            name: input.name,
            names: input
                .names
                .map(|value| -> Result<_, RpcCodecError> { Ok(v1::ListOfString { items: value }) })
                .transpose()?,
            create_missing: Some(input.create_missing),
            actor: input.actor,
        })
    }
}
impl v1::BootstrapTaskLabelRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::TaskLabelSurfacePath,
            (),
            crate::label_surfaces::BootstrapTaskLabelRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::TaskLabelSurfacePath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            crate::label_surfaces::BootstrapTaskLabelRequest {
                name: required(self.name, "name")?,
                description: self.description,
                applies_when: self.applies_when,
                excludes_when: self.excludes_when,
                positive_examples: self.positive_examples,
                negative_examples: self.negative_examples,
                verify: self.verify.unwrap_or_default(),
                min_verify_score: match self.min_verify_score {
                    Some(value) => finite(value)?,
                    None => 0.50,
                },
                vector_config: self
                    .vector_config
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .transpose()?,
                actor: self.actor,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::TaskLabelSurfacePath,
        _query: (),
        input: crate::label_surfaces::BootstrapTaskLabelRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            name: Some(input.name),
            description: input.description,
            applies_when: input.applies_when,
            excludes_when: input.excludes_when,
            positive_examples: input.positive_examples,
            negative_examples: input.negative_examples,
            verify: Some(input.verify),
            min_verify_score: Some(finite(input.min_verify_score)?),
            vector_config: input
                .vector_config
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            actor: input.actor,
        })
    }
}
impl v1::RemoveTaskLabelRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::labels::RemoveTaskLabelPath, (), ()), RpcCodecError> {
        Ok((
            crate::labels::RemoveTaskLabelPath {
                task_id: required(self.task_id, "task_id")?,
                label_id: required(self.label_id, "label_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::labels::RemoveTaskLabelPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            label_id: Some(path.label_id),
        })
    }
}
impl v1::ListBoardLabelsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::BoardLabelPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
        })
    }
}
impl v1::ListBoardLabelProposalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::ListBoardLabelProposalsPath,
            crate::label_surfaces::ListBoardLabelProposalsQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::ListBoardLabelProposalsPath {
                board: required(self.board, "board")?,
            },
            crate::label_surfaces::ListBoardLabelProposalsQuery {
                status: self
                    .status
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoLabelProposalStatusWire::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .transpose()?,
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::ListBoardLabelProposalsPath,
        query: crate::label_surfaces::ListBoardLabelProposalsQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            status: query
                .status
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoLabelProposalStatusWire::try_from(value)?))
                })
                .transpose()?,
        })
    }
}
impl v1::CreateBoardLabelRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::label_surfaces::CreateBoardLabelRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::label_surfaces::CreateBoardLabelRequest {
                name: required(self.name, "name")?,
                color: self.color,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::label_surfaces::CreateBoardLabelRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            name: Some(input.name),
            color: input.color,
        })
    }
}
impl v1::DeleteBoardLabelRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::DeleteBoardLabelPath,
            crate::label_surfaces::DeleteBoardLabelQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::DeleteBoardLabelPath {
                board: required(self.board, "board")?,
                label_id: required(self.label_id, "label_id")?,
            },
            crate::label_surfaces::DeleteBoardLabelQuery {
                force: self.force.unwrap_or_default(),
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::DeleteBoardLabelPath,
        query: crate::label_surfaces::DeleteBoardLabelQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            label_id: Some(path.label_id),
            force: Some(query.force),
        })
    }
}
impl v1::ListLabelSemanticsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::BoardLabelPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
        })
    }
}
impl v1::GetLabelSemanticsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::LabelSemanticsPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::LabelSemanticsPath {
                board: required(self.board, "board")?,
                label_id: required(self.label_id, "label_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::LabelSemanticsPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            label_id: Some(path.label_id),
        })
    }
}
impl v1::UpsertLabelSemanticsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::LabelSemanticsPath,
            (),
            crate::label_surfaces::UpsertLabelSemanticsRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::LabelSemanticsPath {
                board: required(self.board, "board")?,
                label_id: required(self.label_id, "label_id")?,
            },
            (),
            crate::label_surfaces::UpsertLabelSemanticsRequest {
                actor: self.actor,
                expected_semantics_hash: self.expected_semantics_hash,
                replace: self.replace.unwrap_or_default(),
                reason: self.reason,
                source_signal_ids: self.source_signal_ids,
                description: self.description,
                applies_when: self
                    .applies_when
                    .map(|value| -> Result<_, RpcCodecError> { Ok((value).items) })
                    .transpose()?,
                excludes_when: self
                    .excludes_when
                    .map(|value| -> Result<_, RpcCodecError> { Ok((value).items) })
                    .transpose()?,
                positive_examples: self
                    .positive_examples
                    .map(|value| -> Result<_, RpcCodecError> { Ok((value).items) })
                    .transpose()?,
                negative_examples: self
                    .negative_examples
                    .map(|value| -> Result<_, RpcCodecError> { Ok((value).items) })
                    .transpose()?,
                remove_applies_when: self.remove_applies_when,
                remove_excludes_when: self.remove_excludes_when,
                remove_positive_examples: self.remove_positive_examples,
                remove_negative_examples: self.remove_negative_examples,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::LabelSemanticsPath,
        _query: (),
        input: crate::label_surfaces::UpsertLabelSemanticsRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            label_id: Some(path.label_id),
            actor: input.actor,
            expected_semantics_hash: input.expected_semantics_hash,
            replace: Some(input.replace),
            reason: input.reason,
            source_signal_ids: input.source_signal_ids,
            description: input.description,
            applies_when: input
                .applies_when
                .map(|value| -> Result<_, RpcCodecError> { Ok(v1::ListOfString { items: value }) })
                .transpose()?,
            excludes_when: input
                .excludes_when
                .map(|value| -> Result<_, RpcCodecError> { Ok(v1::ListOfString { items: value }) })
                .transpose()?,
            positive_examples: input
                .positive_examples
                .map(|value| -> Result<_, RpcCodecError> { Ok(v1::ListOfString { items: value }) })
                .transpose()?,
            negative_examples: input
                .negative_examples
                .map(|value| -> Result<_, RpcCodecError> { Ok(v1::ListOfString { items: value }) })
                .transpose()?,
            remove_applies_when: input.remove_applies_when,
            remove_excludes_when: input.remove_excludes_when,
            remove_positive_examples: input.remove_positive_examples,
            remove_negative_examples: input.remove_negative_examples,
        })
    }
}
impl v1::DeleteLabelSemanticsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::LabelSemanticsPath,
            crate::label_surfaces::DeleteLabelSemanticsQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::LabelSemanticsPath {
                board: required(self.board, "board")?,
                label_id: required(self.label_id, "label_id")?,
            },
            crate::label_surfaces::DeleteLabelSemanticsQuery {
                expected_semantics_hash: required(
                    self.expected_semantics_hash,
                    "expected_semantics_hash",
                )?,
                reason: required(self.reason, "reason")?,
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::LabelSemanticsPath,
        query: crate::label_surfaces::DeleteLabelSemanticsQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            label_id: Some(path.label_id),
            expected_semantics_hash: Some(query.expected_semantics_hash),
            reason: Some(query.reason),
        })
    }
}
impl v1::ListLabelAtomsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::BoardLabelPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
        })
    }
}
impl v1::ExplainLabelAtomRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::LabelAtomPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::LabelAtomPath {
                board: required(self.board, "board")?,
                atom_ref: required(self.atom_ref, "atom_ref")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::LabelAtomPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            atom_ref: Some(path.atom_ref),
        })
    }
}
impl v1::LabelAtomIndexStatusRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::BoardLabelPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
        })
    }
}
impl v1::RebuildLabelAtomIndexRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::BoardLabelPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
        })
    }
}
impl v1::QueryLabelAtomIndexRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            crate::label_surfaces::LabelAtomIndexQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            crate::label_surfaces::LabelAtomIndexQuery {
                q: self.q,
                vector_json: self.vector_json,
                embedding_model: self.embedding_model,
                include_vector: self.include_vector.unwrap_or_default(),
                polarity: self.polarity,
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 24,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        query: crate::label_surfaces::LabelAtomIndexQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            q: query.q,
            vector_json: query.vector_json,
            embedding_model: query.embedding_model,
            include_vector: Some(query.include_vector),
            polarity: query.polarity,
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::ListSignalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            crate::label_surfaces::SignalQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            crate::label_surfaces::SignalQuery {
                status: self.status,
                kind: self.kind,
                task_ref: self.task_ref,
                include_all: self.include_all.unwrap_or_default(),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 100,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        query: crate::label_surfaces::SignalQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            status: query.status,
            kind: query.kind,
            task_ref: query.task_ref,
            include_all: Some(query.include_all),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::ReviewSignalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            crate::label_surfaces::SignalQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            crate::label_surfaces::SignalQuery {
                status: self.status,
                kind: self.kind,
                task_ref: self.task_ref,
                include_all: self.include_all.unwrap_or_default(),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 100,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        query: crate::label_surfaces::SignalQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            status: query.status,
            kind: query.kind,
            task_ref: query.task_ref,
            include_all: Some(query.include_all),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::GetSignalRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::SignalPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::SignalPath {
                signal_id: required(self.signal_id, "signal_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::SignalPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            signal_id: Some(path.signal_id),
        })
    }
}
impl v1::RecordSignalRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::signals::RecordSignalRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::signals::RecordSignalRequest {
                kind: required(self.kind, "kind")?,
                title: required(self.title, "title")?,
                summary: required(self.summary, "summary")?,
                severity: self.severity,
                task_ref: self.task_ref,
                task_id: self.task_id,
                run_id: self.run_id,
                comment_id: self.comment_id,
                actor: self.actor,
                agent_type: self.agent_type,
                dedupe_key: self.dedupe_key,
                source: self.source,
                evidence: self
                    .evidence
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .transpose()?,
                comment: self
                    .comment
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .transpose()?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::signals::RecordSignalRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            kind: Some(input.kind),
            title: Some(input.title),
            summary: Some(input.summary),
            severity: input.severity,
            task_ref: input.task_ref,
            task_id: input.task_id,
            run_id: input.run_id,
            comment_id: input.comment_id,
            actor: input.actor,
            agent_type: input.agent_type,
            dedupe_key: input.dedupe_key,
            source: input.source,
            evidence: input
                .evidence
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            comment: input
                .comment
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        })
    }
}
impl v1::ConfirmSignalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::signals::ReviewSignalsRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::signals::ReviewSignalsRequest {
                signal_ids: self.signal_ids,
                reason: required(self.reason, "reason")?,
                replacement_signal_id: self.replacement_signal_id,
                actor: self.actor,
                expected_updated_at: self.expected_updated_at,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::signals::ReviewSignalsRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            signal_ids: input.signal_ids,
            reason: Some(input.reason),
            replacement_signal_id: input.replacement_signal_id,
            actor: input.actor,
            expected_updated_at: input.expected_updated_at,
        })
    }
}
impl v1::RejectSignalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::signals::ReviewSignalsRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::signals::ReviewSignalsRequest {
                signal_ids: self.signal_ids,
                reason: required(self.reason, "reason")?,
                replacement_signal_id: self.replacement_signal_id,
                actor: self.actor,
                expected_updated_at: self.expected_updated_at,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::signals::ReviewSignalsRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            signal_ids: input.signal_ids,
            reason: Some(input.reason),
            replacement_signal_id: input.replacement_signal_id,
            actor: input.actor,
            expected_updated_at: input.expected_updated_at,
        })
    }
}
impl v1::ResolveSignalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::signals::ReviewSignalsRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::signals::ReviewSignalsRequest {
                signal_ids: self.signal_ids,
                reason: required(self.reason, "reason")?,
                replacement_signal_id: self.replacement_signal_id,
                actor: self.actor,
                expected_updated_at: self.expected_updated_at,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::signals::ReviewSignalsRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            signal_ids: input.signal_ids,
            reason: Some(input.reason),
            replacement_signal_id: input.replacement_signal_id,
            actor: input.actor,
            expected_updated_at: input.expected_updated_at,
        })
    }
}
impl v1::SupersedeSignalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::signals::ReviewSignalsRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::signals::ReviewSignalsRequest {
                signal_ids: self.signal_ids,
                reason: required(self.reason, "reason")?,
                replacement_signal_id: self.replacement_signal_id,
                actor: self.actor,
                expected_updated_at: self.expected_updated_at,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::signals::ReviewSignalsRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            signal_ids: input.signal_ids,
            reason: Some(input.reason),
            replacement_signal_id: input.replacement_signal_id,
            actor: input.actor,
            expected_updated_at: input.expected_updated_at,
        })
    }
}
impl v1::SuggestTaskLabelsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::TaskLabelSurfacePath,
            crate::rpc::dto::TaskLabelSuggestionQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::TaskLabelSurfacePath {
                task_id: required(self.task_id, "task_id")?,
            },
            crate::rpc::dto::TaskLabelSuggestionQuery {
                board: self.board,
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 5,
                },
                candidate_limit: match self.candidate_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 32,
                },
                atom_limit: match self.atom_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 80,
                },
                max_selected_labels: match self.max_selected_labels {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 4,
                },
                min_score: match self.min_score {
                    Some(value) => finite(value)?,
                    None => 0.15,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::TaskLabelSurfacePath,
        query: crate::rpc::dto::TaskLabelSuggestionQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            board: query.board,
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            candidate_limit: Some(
                u64::try_from(query.candidate_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            atom_limit: Some(
                u64::try_from(query.atom_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            max_selected_labels: Some(
                u64::try_from(query.max_selected_labels)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            min_score: Some(finite(query.min_score)?),
        })
    }
}
impl v1::ListTaskLabelProposalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::TaskLabelSurfacePath,
            crate::rpc::dto::TaskLabelProposalQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::TaskLabelSurfacePath {
                task_id: required(self.task_id, "task_id")?,
            },
            crate::rpc::dto::TaskLabelProposalQuery {
                board: self.board,
                status: self.status,
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::TaskLabelSurfacePath,
        query: crate::rpc::dto::TaskLabelProposalQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            board: query.board,
            status: query.status,
        })
    }
}
impl v1::ProposeTaskLabelRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::TaskLabelSurfacePath,
            crate::rpc::dto::TaskLabelSuggestionQuery,
            crate::label_surfaces::ProposeTaskLabelRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::TaskLabelSurfacePath {
                task_id: required(self.task_id, "task_id")?,
            },
            crate::rpc::dto::TaskLabelSuggestionQuery {
                board: self.board,
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 5,
                },
                candidate_limit: match self.candidate_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 32,
                },
                atom_limit: match self.atom_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 80,
                },
                max_selected_labels: match self.max_selected_labels {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 4,
                },
                min_score: match self.min_score {
                    Some(value) => finite(value)?,
                    None => 0.15,
                },
            },
            crate::label_surfaces::ProposeTaskLabelRequest {
                proposal: self
                    .proposal
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .transpose()?,
                actor: self.actor,
                source_signal_ids: self.source_signal_ids,
                ontology_actor: self
                    .ontology_actor
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .transpose()?,
                allow_retarget: self.allow_retarget.unwrap_or_default(),
                retarget_reason: self.retarget_reason,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::TaskLabelSurfacePath,
        query: crate::rpc::dto::TaskLabelSuggestionQuery,
        input: crate::label_surfaces::ProposeTaskLabelRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            board: query.board,
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            candidate_limit: Some(
                u64::try_from(query.candidate_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            atom_limit: Some(
                u64::try_from(query.atom_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            max_selected_labels: Some(
                u64::try_from(query.max_selected_labels)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            min_score: Some(finite(query.min_score)?),
            proposal: input
                .proposal
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            actor: input.actor,
            source_signal_ids: input.source_signal_ids,
            ontology_actor: input
                .ontology_actor
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            allow_retarget: Some(input.allow_retarget),
            retarget_reason: input.retarget_reason,
        })
    }
}
impl v1::RecordLabelOntologyObservationRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::TaskLabelSurfacePath,
            crate::rpc::dto::TaskLabelBoardQuery,
            crate::label_surfaces::RecordLabelOntologyObservationRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::TaskLabelSurfacePath {
                task_id: required(self.task_id, "task_id")?,
            },
            crate::rpc::dto::TaskLabelBoardQuery { board: self.board },
            crate::label_surfaces::RecordLabelOntologyObservationRequest {
                actor: (required(self.actor, "actor")?).try_into()?,
                agent_candidates: match self.agent_candidates {
                    None => crate::JsonBodyFieldWire::Missing,
                    Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
                },
                suggestion_snapshot: match self.suggestion_snapshot {
                    None => crate::JsonBodyFieldWire::Missing,
                    Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
                },
                final_decision: match self.final_decision {
                    None => crate::JsonBodyFieldWire::Missing,
                    Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
                },
                suggest_coverage: self
                    .suggest_coverage
                    .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                    .transpose()?,
                suggest_coverage_cosine: self
                    .suggest_coverage_cosine
                    .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                    .transpose()?,
                suggest_residual_norm: self
                    .suggest_residual_norm
                    .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                    .transpose()?,
                suggest_needs_new_label: self.suggest_needs_new_label,
                suggest_degraded: self.suggest_degraded,
                diagnostics: match self.diagnostics {
                    None => crate::JsonBodyFieldWire::Missing,
                    Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
                },
                capture_fingerprint: self.capture_fingerprint,
                signals: (self.signals)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .collect::<Result<Vec<_>, _>>()?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::TaskLabelSurfacePath,
        query: crate::rpc::dto::TaskLabelBoardQuery,
        input: crate::label_surfaces::RecordLabelOntologyObservationRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            board: query.board,
            actor: Some((input.actor).try_into()?),
            agent_candidates: match input.agent_candidates {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
            suggestion_snapshot: match input.suggestion_snapshot {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
            final_decision: match input.final_decision {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
            suggest_coverage: input
                .suggest_coverage
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_coverage_cosine: input
                .suggest_coverage_cosine
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_residual_norm: input
                .suggest_residual_norm
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_needs_new_label: input.suggest_needs_new_label,
            suggest_degraded: input.suggest_degraded,
            diagnostics: match input.diagnostics {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
            capture_fingerprint: input.capture_fingerprint,
            signals: (input.signals)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl v1::ListLabelOntologySignalsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            crate::label_surfaces::LabelOntologySignalQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            crate::label_surfaces::LabelOntologySignalQuery {
                status: self.status,
                kind: self.kind,
                task_ref: self.task_ref,
                target_label_ref: self.target_label_ref,
                proposed_label_name: self.proposed_label_name,
                include_all: self.include_all.unwrap_or_default(),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 100,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        query: crate::label_surfaces::LabelOntologySignalQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            status: query.status,
            kind: query.kind,
            task_ref: query.task_ref,
            target_label_ref: query.target_label_ref,
            proposed_label_name: query.proposed_label_name,
            include_all: Some(query.include_all),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::ReviewLabelOntologyRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            crate::label_surfaces::LabelOntologyReviewQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            crate::label_surfaces::LabelOntologyReviewQuery {
                group_by: match self.group_by {
                    Some(value) => v1::DtoLabelOntologyReviewGroupByWire::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                        .try_into()?,
                    None => crate::label_surfaces::LabelOntologyReviewGroupByWire::Label,
                },
                include_all: self.include_all.unwrap_or_default(),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 100,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        query: crate::label_surfaces::LabelOntologyReviewQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            group_by: Some(i32::from(v1::DtoLabelOntologyReviewGroupByWire::try_from(
                query.group_by,
            )?)),
            include_all: Some(query.include_all),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::CreateLabelOntologyActionRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::label_surfaces::LabelOntologyActionRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::label_surfaces::LabelOntologyActionRequest {
                actor: (required(self.actor, "actor")?).try_into()?,
                idempotency_key: self.idempotency_key,
                action_type: v1::DtoLabelOntologyActionTypeWire::try_from(required(
                    self.action_type,
                    "action_type",
                )?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
                signal_ids: self.signal_ids,
                reason: required(self.reason, "reason")?,
                superseded_by_signal_id: self.superseded_by_signal_id,
                parent_action_id: self.parent_action_id,
                target_label_ref: self.target_label_ref,
                result_label_ref: self.result_label_ref,
                result_atom_id: self.result_atom_id,
                result_atom_content_hash: self.result_atom_content_hash,
                result_proposal_id: self.result_proposal_id,
                canonical_before_hash: self.canonical_before_hash,
                canonical_after_hash: self.canonical_after_hash,
                change: match self.change {
                    None => crate::JsonBodyFieldWire::Missing,
                    Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
                },
                validation_status: self
                    .validation_status
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoLabelOntologyValidationStatusWire::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .transpose()?,
                validation: match self.validation {
                    None => crate::JsonBodyFieldWire::Missing,
                    Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
                },
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::label_surfaces::LabelOntologyActionRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            actor: Some((input.actor).try_into()?),
            idempotency_key: input.idempotency_key,
            action_type: Some(i32::from(v1::DtoLabelOntologyActionTypeWire::try_from(
                input.action_type,
            )?)),
            signal_ids: input.signal_ids,
            reason: Some(input.reason),
            superseded_by_signal_id: input.superseded_by_signal_id,
            parent_action_id: input.parent_action_id,
            target_label_ref: input.target_label_ref,
            result_label_ref: input.result_label_ref,
            result_atom_id: input.result_atom_id,
            result_atom_content_hash: input.result_atom_content_hash,
            result_proposal_id: input.result_proposal_id,
            canonical_before_hash: input.canonical_before_hash,
            canonical_after_hash: input.canonical_after_hash,
            change: match input.change {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
            validation_status: input
                .validation_status
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(
                        v1::DtoLabelOntologyValidationStatusWire::try_from(value)?,
                    ))
                })
                .transpose()?,
            validation: match input.validation {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
        })
    }
}
impl v1::ApplyLabelOntologyAtomRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::label_surfaces::ApplyLabelOntologyAtomRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::label_surfaces::ApplyLabelOntologyAtomRequest {
                actor: (required(self.actor, "actor")?).try_into()?,
                signal_ids: self.signal_ids,
                label_ref: required(self.label_ref, "label_ref")?,
                kind: required(self.kind, "kind")?,
                text: required(self.text, "text")?,
                reason: required(self.reason, "reason")?,
                allow_retarget: self.allow_retarget.unwrap_or_default(),
                retarget_reason: self.retarget_reason,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::label_surfaces::ApplyLabelOntologyAtomRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            actor: Some((input.actor).try_into()?),
            signal_ids: input.signal_ids,
            label_ref: Some(input.label_ref),
            kind: Some(input.kind),
            text: Some(input.text),
            reason: Some(input.reason),
            allow_retarget: Some(input.allow_retarget),
            retarget_reason: input.retarget_reason,
        })
    }
}
impl v1::RevertLabelOntologyMutationRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::label_surfaces::RevertLabelOntologyMutationRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::label_surfaces::RevertLabelOntologyMutationRequest {
                actor: (required(self.actor, "actor")?).try_into()?,
                target_action_id: required(self.target_action_id, "target_action_id")?,
                expected_current_hash: self.expected_current_hash,
                reason: required(self.reason, "reason")?,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::label_surfaces::RevertLabelOntologyMutationRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            actor: Some((input.actor).try_into()?),
            target_action_id: Some(input.target_action_id),
            expected_current_hash: input.expected_current_hash,
            reason: Some(input.reason),
        })
    }
}
impl v1::ValidateLabelOntologyActionRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            (),
            crate::label_surfaces::ValidateLabelOntologyActionRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            (),
            crate::label_surfaces::ValidateLabelOntologyActionRequest {
                actor: (required(self.actor, "actor")?).try_into()?,
                parent_action_id: required(self.parent_action_id, "parent_action_id")?,
                signal_ids: self.signal_ids,
                reason: required(self.reason, "reason")?,
                validation_status: v1::DtoLabelOntologyValidationStatusWire::try_from(required(
                    self.validation_status,
                    "validation_status",
                )?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
                validation: match self.validation {
                    None => crate::JsonBodyFieldWire::Missing,
                    Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
                },
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        _query: (),
        input: crate::label_surfaces::ValidateLabelOntologyActionRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            actor: Some((input.actor).try_into()?),
            parent_action_id: Some(input.parent_action_id),
            signal_ids: input.signal_ids,
            reason: Some(input.reason),
            validation_status: Some(i32::from(
                v1::DtoLabelOntologyValidationStatusWire::try_from(input.validation_status)?,
            )),
            validation: match input.validation {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
        })
    }
}
impl v1::GetLabelOntologySignalRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::SignalPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::SignalPath {
                signal_id: required(self.signal_id, "signal_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::SignalPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            signal_id: Some(path.signal_id),
        })
    }
}
impl v1::GetLabelProposalRequest {
    pub fn decode_parts(
        self,
    ) -> Result<(crate::label_surfaces::ProposalPath, (), ()), RpcCodecError> {
        Ok((
            crate::label_surfaces::ProposalPath {
                proposal_id: required(self.proposal_id, "proposal_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::ProposalPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            proposal_id: Some(path.proposal_id),
        })
    }
}
impl v1::AcceptLabelProposalRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::ProposalPath,
            (),
            crate::label_surfaces::LabelProposalDecisionRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::ProposalPath {
                proposal_id: required(self.proposal_id, "proposal_id")?,
            },
            (),
            crate::label_surfaces::LabelProposalDecisionRequest {
                reason: self.reason,
                actor: self.actor,
                source_signal_ids: self.source_signal_ids,
                ontology_actor: self
                    .ontology_actor
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .transpose()?,
                allow_retarget: self.allow_retarget.unwrap_or_default(),
                retarget_reason: self.retarget_reason,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::ProposalPath,
        _query: (),
        input: crate::label_surfaces::LabelProposalDecisionRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            proposal_id: Some(path.proposal_id),
            reason: input.reason,
            actor: input.actor,
            source_signal_ids: input.source_signal_ids,
            ontology_actor: input
                .ontology_actor
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            allow_retarget: Some(input.allow_retarget),
            retarget_reason: input.retarget_reason,
        })
    }
}
impl v1::RejectLabelProposalRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::ProposalPath,
            (),
            crate::label_surfaces::LabelProposalDecisionRequest,
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::ProposalPath {
                proposal_id: required(self.proposal_id, "proposal_id")?,
            },
            (),
            crate::label_surfaces::LabelProposalDecisionRequest {
                reason: self.reason,
                actor: self.actor,
                source_signal_ids: self.source_signal_ids,
                ontology_actor: self
                    .ontology_actor
                    .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                    .transpose()?,
                allow_retarget: self.allow_retarget.unwrap_or_default(),
                retarget_reason: self.retarget_reason,
            },
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::ProposalPath,
        _query: (),
        input: crate::label_surfaces::LabelProposalDecisionRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            proposal_id: Some(path.proposal_id),
            reason: input.reason,
            actor: input.actor,
            source_signal_ids: input.source_signal_ids,
            ontology_actor: input
                .ontology_actor
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            allow_retarget: Some(input.allow_retarget),
            retarget_reason: input.retarget_reason,
        })
    }
}
impl v1::BoardTaskMapRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::task_graph::BoardTaskMapPath,
            crate::task_graph::BoardTaskMapQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::task_graph::BoardTaskMapPath {
                board: required(self.board, "board")?,
            },
            crate::task_graph::BoardTaskMapQuery {
                active_only: self.active_only.unwrap_or_else(|| {
                    <crate::task_graph::BoardTaskMapQuery>::default().active_only
                }),
                context_depth: match self.context_depth {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_graph::BoardTaskMapQuery>::default().context_depth,
                },
                limit_nodes: match self.limit_nodes {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_graph::BoardTaskMapQuery>::default().limit_nodes,
                },
                include_done_context: self.include_done_context.unwrap_or_else(|| {
                    <crate::task_graph::BoardTaskMapQuery>::default().include_done_context
                }),
                include_archived_context: self.include_archived_context.unwrap_or_else(|| {
                    <crate::task_graph::BoardTaskMapQuery>::default().include_archived_context
                }),
                hide_isolated: self.hide_isolated.unwrap_or_else(|| {
                    <crate::task_graph::BoardTaskMapQuery>::default().hide_isolated
                }),
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::task_graph::BoardTaskMapPath,
        query: crate::task_graph::BoardTaskMapQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            active_only: Some(query.active_only),
            context_depth: Some(
                u64::try_from(query.context_depth)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            limit_nodes: Some(
                u64::try_from(query.limit_nodes)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            include_done_context: Some(query.include_done_context),
            include_archived_context: Some(query.include_archived_context),
            hide_isolated: Some(query.hide_isolated),
        })
    }
}
impl v1::TaskNeighborhoodRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::task_graph::TaskNeighborhoodPath,
            crate::task_graph::TaskNeighborhoodQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::task_graph::TaskNeighborhoodPath {
                task_id: required(self.task_id, "task_id")?,
            },
            crate::task_graph::TaskNeighborhoodQuery {
                depth: match self.depth {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_graph::TaskNeighborhoodQuery>::default().depth,
                },
                limit_nodes: match self.limit_nodes {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::task_graph::TaskNeighborhoodQuery>::default().limit_nodes,
                },
                include_archived_context: self.include_archived_context.unwrap_or_else(|| {
                    <crate::task_graph::TaskNeighborhoodQuery>::default().include_archived_context
                }),
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::task_graph::TaskNeighborhoodPath,
        query: crate::task_graph::TaskNeighborhoodQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            depth: Some(
                u64::try_from(query.depth)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            limit_nodes: Some(
                u64::try_from(query.limit_nodes)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            include_archived_context: Some(query.include_archived_context),
        })
    }
}
impl v1::SearchTasksRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::SearchTasksQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::SearchTasksQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
                q: self.q,
                status: (self.status)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoApiTaskStatus::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                label: self.label,
                include_archived: self.include_archived.unwrap_or_default(),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 20,
                },
                offset: match self.offset {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => Default::default(),
                },
                assignee: self.assignee,
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::SearchTasksQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
            q: query.q,
            status: (query.status)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoApiTaskStatus::try_from(value)?))
                })
                .collect::<Result<Vec<_>, _>>()?,
            label: query.label,
            include_archived: Some(query.include_archived),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            offset: Some(
                u64::try_from(query.offset)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            assignee: query.assignee,
        })
    }
}
impl v1::SearchTasksByStatusRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::SearchTasksQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::SearchTasksQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
                q: self.q,
                status: (self.status)
                    .into_iter()
                    .map(|value| -> Result<_, RpcCodecError> {
                        v1::DtoApiTaskStatus::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                            .try_into()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                label: self.label,
                include_archived: self.include_archived.unwrap_or_default(),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 20,
                },
                offset: match self.offset {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => Default::default(),
                },
                assignee: self.assignee,
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::SearchTasksQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
            q: query.q,
            status: (query.status)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoApiTaskStatus::try_from(value)?))
                })
                .collect::<Result<Vec<_>, _>>()?,
            label: query.label,
            include_archived: Some(query.include_archived),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            offset: Some(
                u64::try_from(query.offset)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            assignee: query.assignee,
        })
    }
}
impl v1::SearchStatusRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::BoardQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::BoardQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::BoardQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::RebuildSearchIndexRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::BoardQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::BoardQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::BoardQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::SyncSearchIndexRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::BoardQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::BoardQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::BoardQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::BuildContextRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::derived::BuildContextPath,
            crate::derived::BuildContextQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::derived::BuildContextPath {
                task_id: required(self.task_id, "task_id")?,
            },
            crate::derived::BuildContextQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
                lexical_limit: match self.lexical_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 5,
                },
                graph_limit: match self.graph_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 10,
                },
                vector_limit: match self.vector_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 5,
                },
                max_items: match self.max_items {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 20,
                },
                task: self.task,
                reference: self.reference,
                query: self.query,
                depth: match self.depth {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 1,
                },
                budget: self
                    .budget
                    .map(|value| -> Result<_, RpcCodecError> {
                        usize::try_from(value)
                            .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))
                    })
                    .transpose()?,
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::derived::BuildContextPath,
        query: crate::derived::BuildContextQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
            board: Some(query.board),
            lexical_limit: Some(
                u64::try_from(query.lexical_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            graph_limit: Some(
                u64::try_from(query.graph_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            vector_limit: Some(
                u64::try_from(query.vector_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            max_items: Some(
                u64::try_from(query.max_items)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            task: query.task,
            reference: query.reference,
            query: query.query,
            depth: Some(
                u64::try_from(query.depth)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            budget: query
                .budget
                .map(|value| -> Result<_, RpcCodecError> {
                    u64::try_from(value).map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))
                })
                .transpose()?,
        })
    }
}
impl v1::GraphStatusRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::BoardQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::BoardQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::BoardQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::GraphNeighborsRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), crate::derived::GraphNeighborsQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::GraphNeighborsQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
                entity_uri: required(self.entity_uri, "entity_uri")?,
                predicate: self.predicate,
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 50,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::GraphNeighborsQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
            entity_uri: Some(query.entity_uri),
            predicate: query.predicate,
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::GraphQueryRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::GraphQueryQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::GraphQueryQuery {
                board: self
                    .board
                    .unwrap_or_else(|| <crate::derived::GraphQueryQuery>::default().board),
                query: self
                    .query
                    .unwrap_or_else(|| <crate::derived::GraphQueryQuery>::default().query),
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::derived::GraphQueryQuery>::default().limit,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::GraphQueryQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
            query: Some(query.query),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::GraphRebuildRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::BoardQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::BoardQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::BoardQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::GraphSyncRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::BoardQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::BoardQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::BoardQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::ListEntitiesRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::EntityListQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::EntityListQuery {
                board: self.board,
                kind: self.kind,
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => <crate::derived::EntityListQuery>::default().limit,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::EntityListQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: query.board,
            kind: query.kind,
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::UpsertEntityRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::derived::EntityUpsertRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::derived::EntityUpsertRequest {
                uri: required(self.uri, "uri")?,
                kind: required(self.kind, "kind")?,
                source_table: required(self.source_table, "source_table")?,
                source_id: required(self.source_id, "source_id")?,
                board: self.board,
                task_id: self.task_id,
                title: self.title,
                summary: self.summary,
                content_hash: self.content_hash,
                archived_at: self.archived_at,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::derived::EntityUpsertRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            uri: Some(input.uri),
            kind: Some(input.kind),
            source_table: Some(input.source_table),
            source_id: Some(input.source_id),
            board: input.board,
            task_id: input.task_id,
            title: input.title,
            summary: input.summary,
            content_hash: input.content_hash,
            archived_at: input.archived_at,
        })
    }
}
impl v1::GetEntityRequest {
    pub fn decode_parts(self) -> Result<(crate::derived::EntityPath, (), ()), RpcCodecError> {
        Ok((
            crate::derived::EntityPath {
                uri: required(self.uri, "uri")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::derived::EntityPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            uri: Some(path.uri),
        })
    }
}
impl v1::VectorStatusRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), crate::protocols::VectorStatusQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::protocols::VectorStatusQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::protocols::VectorStatusQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::VectorConfigureRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::protocols::VectorConfigureRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::protocols::VectorConfigureRequest {
                provider: required(self.provider, "provider")?,
                endpoint: required(self.endpoint, "endpoint")?,
                model: required(self.model, "model")?,
                dimensions: usize::try_from(required(self.dimensions, "dimensions")?)
                    .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::protocols::VectorConfigureRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            provider: Some(input.provider),
            endpoint: Some(input.endpoint),
            model: Some(input.model),
            dimensions: Some(
                u64::try_from(input.dimensions)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl v1::VectorRebuildRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::protocols::VectorProjectionRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::protocols::VectorProjectionRequest {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::protocols::VectorProjectionRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(input.board),
        })
    }
}
impl v1::VectorSyncRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::protocols::VectorProjectionRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::protocols::VectorProjectionRequest {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::protocols::VectorProjectionRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(input.board),
        })
    }
}
impl v1::VectorQueryChunksRequest {
    pub fn decode_parts(self) -> Result<((), crate::protocols::VectorQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::protocols::VectorQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
                q: required(self.q, "q")?,
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 20,
                },
                embedding_model: self.embedding_model,
                polarity: self.polarity,
                include_vector: self.include_vector.unwrap_or_default(),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::protocols::VectorQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
            q: Some(query.q),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            embedding_model: query.embedding_model,
            polarity: query.polarity,
            include_vector: Some(query.include_vector),
        })
    }
}
impl v1::VectorQueryLabelAtomsRequest {
    pub fn decode_parts(self) -> Result<((), crate::protocols::VectorQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::protocols::VectorQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
                q: required(self.q, "q")?,
                limit: match self.limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 20,
                },
                embedding_model: self.embedding_model,
                polarity: self.polarity,
                include_vector: self.include_vector.unwrap_or_default(),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::protocols::VectorQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
            q: Some(query.q),
            limit: Some(
                u64::try_from(query.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            embedding_model: query.embedding_model,
            polarity: query.polarity,
            include_vector: Some(query.include_vector),
        })
    }
}
impl v1::GetStatsRequest {
    pub fn decode_parts(self) -> Result<((), crate::derived::BoardQuery, ()), RpcCodecError> {
        Ok((
            (),
            crate::derived::BoardQuery {
                board: self.board.unwrap_or_else(|| "default".to_owned()),
            },
            (),
        ))
    }
    pub fn from_parts(
        _path: (),
        query: crate::derived::BoardQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(query.board),
        })
    }
}
impl v1::DoctorRequest {
    pub fn decode_parts(self) -> Result<((), (), ()), RpcCodecError> {
        Ok(((), (), ()))
    }
    pub fn from_parts(_path: (), _query: (), _input: ()) -> Result<Self, RpcCodecError> {
        Ok(Self {})
    }
}
impl v1::CheckpointRequest {
    pub fn decode_parts(self) -> Result<((), (), ()), RpcCodecError> {
        Ok(((), (), ()))
    }
    pub fn from_parts(_path: (), _query: (), _input: ()) -> Result<Self, RpcCodecError> {
        Ok(Self {})
    }
}
impl v1::MaintenanceBackupRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::maintenance::MaintenancePathRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::maintenance::MaintenancePathRequest {
                path: required(self.path, "path")?,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::maintenance::MaintenancePathRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            path: Some(input.path),
        })
    }
}
impl v1::MaintenanceExportRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::maintenance::MaintenancePathRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::maintenance::MaintenancePathRequest {
                path: required(self.path, "path")?,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::maintenance::MaintenancePathRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            path: Some(input.path),
        })
    }
}
impl v1::MaintenanceImportRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::maintenance::MaintenanceImportRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::maintenance::MaintenanceImportRequest {
                path: required(self.path, "path")?,
                replace: self.replace.unwrap_or_default(),
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::maintenance::MaintenanceImportRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            path: Some(input.path),
            replace: Some(input.replace),
        })
    }
}
impl v1::MaintenanceVacuumRequest {
    pub fn decode_parts(self) -> Result<((), (), ()), RpcCodecError> {
        Ok(((), (), ()))
    }
    pub fn from_parts(_path: (), _query: (), _input: ()) -> Result<Self, RpcCodecError> {
        Ok(Self {})
    }
}
impl v1::MaintenanceStatusRequest {
    pub fn decode_parts(self) -> Result<((), (), ()), RpcCodecError> {
        Ok(((), (), ()))
    }
    pub fn from_parts(_path: (), _query: (), _input: ()) -> Result<Self, RpcCodecError> {
        Ok(Self {})
    }
}
impl v1::MaintenanceRunRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::maintenance::MaintenanceRunRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::maintenance::MaintenanceRunRequest {
                owner: self.owner,
                action: self.action,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::maintenance::MaintenanceRunRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            owner: input.owner,
            action: input.action,
        })
    }
}
impl v1::MaintenanceRebuildRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::maintenance::MaintenanceRunRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::maintenance::MaintenanceRunRequest {
                owner: self.owner,
                action: self.action,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::maintenance::MaintenanceRunRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            owner: input.owner,
            action: input.action,
        })
    }
}
impl v1::MaintenanceCleanupRequest {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::maintenance::MaintenanceRunRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::maintenance::MaintenanceRunRequest {
                owner: self.owner,
                action: self.action,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::maintenance::MaintenanceRunRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            owner: input.owner,
            action: input.action,
        })
    }
}
impl v1::MaintenanceImportV30Request {
    pub fn decode_parts(
        self,
    ) -> Result<((), (), crate::maintenance::LegacyImportRequest), RpcCodecError> {
        Ok((
            (),
            (),
            crate::maintenance::LegacyImportRequest {
                path: required(self.path, "path")?,
                canonical_attachment_root: self.canonical_attachment_root,
            },
        ))
    }
    pub fn from_parts(
        _path: (),
        _query: (),
        input: crate::maintenance::LegacyImportRequest,
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            path: Some(input.path),
            canonical_attachment_root: input.canonical_attachment_root,
        })
    }
}
impl v1::GetTaskDetailsRequest {
    pub fn decode_parts(self) -> Result<(crate::task_core::GetTaskPath, (), ()), RpcCodecError> {
        Ok((
            crate::task_core::GetTaskPath {
                task_id: required(self.task_id, "task_id")?,
            },
            (),
            (),
        ))
    }
    pub fn from_parts(
        path: crate::task_core::GetTaskPath,
        _query: (),
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            task_id: Some(path.task_id),
        })
    }
}
impl v1::GetLabelOntologyQualityRequest {
    pub fn decode_parts(
        self,
    ) -> Result<
        (
            crate::label_surfaces::BoardLabelPath,
            crate::rpc::dto::LabelOntologyQualityQuery,
            (),
        ),
        RpcCodecError,
    > {
        Ok((
            crate::label_surfaces::BoardLabelPath {
                board: required(self.board, "board")?,
            },
            crate::rpc::dto::LabelOntologyQualityQuery {
                sample_limit: match self.sample_limit {
                    Some(value) => usize::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                    None => 20,
                },
            },
            (),
        ))
    }
    pub fn from_parts(
        path: crate::label_surfaces::BoardLabelPath,
        query: crate::rpc::dto::LabelOntologyQualityQuery,
        _input: (),
    ) -> Result<Self, RpcCodecError> {
        Ok(Self {
            board: Some(path.board),
            sample_limit: Some(
                u64::try_from(query.sample_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>>
    for v1::AcceptLabelProposalResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::AcceptLabelProposalResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::AcceptLabelProposalResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::dependencies::AddDependencyResponse> for v1::AddDependencyResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::dependencies::AddDependencyResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::AddDependencyResponse> for crate::dependencies::AddDependencyResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::AddDependencyResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::dependencies::AddDependencyResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<
        crate::wire::OptionalMetadataEnvelope<
            crate::api_components::ApiTask,
            crate::wire::CreatedLabelsMeta<crate::api_components::ApiLabel>,
        >,
    > for v1::AddTaskLabelResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::OptionalMetadataEnvelope<
            crate::api_components::ApiTask,
            crate::wire::CreatedLabelsMeta<crate::api_components::ApiLabel>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
            meta: dto
                .meta
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::AddTaskLabelResponse>
    for crate::wire::OptionalMetadataEnvelope<
        crate::api_components::ApiTask,
        crate::wire::CreatedLabelsMeta<crate::api_components::ApiLabel>,
    >
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::AddTaskLabelResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::OptionalMetadataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
            meta: wire
                .meta
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>>
    for v1::ApplyLabelOntologyAtomResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ApplyLabelOntologyAtomResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ApplyLabelOntologyAtomResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::boards::ArchiveBoardResponse> for v1::ArchiveBoardResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::boards::ArchiveBoardResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ArchiveBoardResponse> for crate::boards::ArchiveBoardResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ArchiveBoardResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::boards::ArchiveBoardResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::ArchiveTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ArchiveTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ArchiveTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>> for v1::BlockTaskResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::BlockTaskResponse> for crate::wire::DataEnvelope<crate::api_components::ApiTask> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::BlockTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::task_graph::BoardTaskMap>>
    for v1::BoardTaskMapResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::task_graph::BoardTaskMap>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::BoardTaskMapResponse>
    for crate::wire::DataEnvelope<crate::task_graph::BoardTaskMap>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::BoardTaskMapResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::BootstrapTaskLabelData>>
    for v1::BootstrapTaskLabelResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::BootstrapTaskLabelData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::BootstrapTaskLabelResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::BootstrapTaskLabelData>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::BootstrapTaskLabelResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::ContextPack>> for v1::BuildContextResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::ContextPack>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::BuildContextResponse> for crate::wire::DataEnvelope<crate::derived::ContextPack> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::BuildContextResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::CheckpointReport>>
    for v1::CheckpointResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::CheckpointReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CheckpointResponse>
    for crate::wire::DataEnvelope<crate::maintenance::CheckpointReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::CheckpointResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::runs::ApiClaim>> for v1::ClaimTaskResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::runs::ApiClaim>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ClaimTaskResponse> for crate::wire::DataEnvelope<crate::runs::ApiClaim> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ClaimTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::CompleteStepResponse> for v1::CompleteStepResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::CompleteStepResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CompleteStepResponse> for crate::steps::CompleteStepResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::CompleteStepResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::CompleteStepResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::CompleteTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CompleteTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::CompleteTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>>
    for v1::ConfirmSignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ConfirmSignalsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ConfirmSignalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::attachments::ApiAttachment>>
    for v1::CreateAttachmentResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::attachments::ApiAttachment>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CreateAttachmentResponse>
    for crate::wire::DataEnvelope<crate::attachments::ApiAttachment>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::CreateAttachmentResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiLabel>>
    for v1::CreateBoardLabelResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiLabel>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CreateBoardLabelResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiLabel>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::CreateBoardLabelResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::boards::CreateBoardResponse> for v1::CreateBoardResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::boards::CreateBoardResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CreateBoardResponse> for crate::boards::CreateBoardResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::CreateBoardResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::boards::CreateBoardResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::comments::CreateCommentResponse> for v1::CreateCommentResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::comments::CreateCommentResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CreateCommentResponse> for crate::comments::CreateCommentResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::CreateCommentResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::comments::CreateCommentResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>>
    for v1::CreateLabelOntologyActionResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CreateLabelOntologyActionResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::CreateLabelOntologyActionResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::CreateStepResponse> for v1::CreateStepResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::CreateStepResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CreateStepResponse> for crate::steps::CreateStepResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::CreateStepResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::CreateStepResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::create_task::CreateTaskResponse> for v1::CreateTaskResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::create_task::CreateTaskResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::CreateTaskResponse> for crate::create_task::CreateTaskResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::CreateTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::create_task::CreateTaskResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::wire::DeleteResult>>
    for v1::DeleteAttachmentResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::wire::DeleteResult>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::DeleteAttachmentResponse>
    for crate::wire::DataEnvelope<crate::wire::DeleteResult>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DeleteAttachmentResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::DeleteBoardLabelResult>>
    for v1::DeleteBoardLabelResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::DeleteBoardLabelResult>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::DeleteBoardLabelResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::DeleteBoardLabelResult>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DeleteBoardLabelResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::wire::DeleteResult>>
    for v1::DeleteLabelSemanticsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::wire::DeleteResult>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::DeleteLabelSemanticsResponse>
    for crate::wire::DataEnvelope<crate::wire::DeleteResult>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DeleteLabelSemanticsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::DoctorReport>> for v1::DoctorResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::DoctorReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::DoctorResponse> for crate::wire::DataEnvelope<crate::maintenance::DoctorReport> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DoctorResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::rpc::dto::AttachmentDownload> for v1::DownloadAttachmentResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::rpc::dto::AttachmentDownload) -> Result<Self, Self::Error> {
        Ok(Self {
            attachment: Some((dto.attachment).try_into()?),
            content: dto.content,
        })
    }
}
impl TryFrom<v1::DownloadAttachmentResponse> for crate::rpc::dto::AttachmentDownload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DownloadAttachmentResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::rpc::dto::AttachmentDownload {
            attachment: (required(wire.attachment, "attachment")?).try_into()?,
            content: wire.content,
        };
        Ok(result)
    }
}
impl TryFrom<crate::attachments::ApiAttachment> for v1::DtoApiAttachment {
    type Error = RpcCodecError;
    fn try_from(dto: crate::attachments::ApiAttachment) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            task_id: Some(dto.task_id),
            filename: Some(dto.filename),
            rel_path: Some(dto.rel_path),
            content_type: dto.content_type,
            size_bytes: Some(dto.size_bytes),
            sha256: dto.sha256,
            created_by: Some(dto.created_by),
            created_at: Some(dto.created_at),
        })
    }
}
impl TryFrom<v1::DtoApiAttachment> for crate::attachments::ApiAttachment {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiAttachment) -> Result<Self, Self::Error> {
        let result: Self = crate::attachments::ApiAttachment {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            task_id: required(wire.task_id, "task_id")?,
            filename: required(wire.filename, "filename")?,
            rel_path: required(wire.rel_path, "rel_path")?,
            content_type: wire.content_type,
            size_bytes: required(wire.size_bytes, "size_bytes")?,
            sha256: wire.sha256,
            created_by: required(wire.created_by, "created_by")?,
            created_at: required(wire.created_at, "created_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::boards::ApiBoard> for v1::DtoApiBoard {
    type Error = RpcCodecError;
    fn try_from(dto: crate::boards::ApiBoard) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            slug: Some(dto.slug),
            name: Some(dto.name),
            description: dto.description,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            archived_at: dto.archived_at,
        })
    }
}
impl TryFrom<v1::DtoApiBoard> for crate::boards::ApiBoard {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiBoard) -> Result<Self, Self::Error> {
        let result: Self = crate::boards::ApiBoard {
            id: required(wire.id, "id")?,
            slug: required(wire.slug, "slug")?,
            name: required(wire.name, "name")?,
            description: wire.description,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            archived_at: wire.archived_at,
        };
        Ok(result)
    }
}
impl TryFrom<crate::boards::ApiBoardColumn> for v1::DtoApiBoardColumn {
    type Error = RpcCodecError;
    fn try_from(dto: crate::boards::ApiBoardColumn) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            status: Some(i32::from(v1::DtoApiTaskStatus::try_from(dto.status)?)),
            title: Some(dto.title),
            position: Some(dto.position),
            hidden: Some(dto.hidden),
            wip_limit: dto.wip_limit,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
        })
    }
}
impl TryFrom<v1::DtoApiBoardColumn> for crate::boards::ApiBoardColumn {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiBoardColumn) -> Result<Self, Self::Error> {
        let result: Self = crate::boards::ApiBoardColumn {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            status: v1::DtoApiTaskStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            title: required(wire.title, "title")?,
            position: required(wire.position, "position")?,
            hidden: required(wire.hidden, "hidden")?,
            wip_limit: wire.wip_limit,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::runs::ApiClaim> for v1::DtoApiClaim {
    type Error = RpcCodecError;
    fn try_from(dto: crate::runs::ApiClaim) -> Result<Self, Self::Error> {
        Ok(Self {
            task: Some((dto.task).try_into()?),
            run: Some((dto.run).try_into()?),
            claim_token: Some(dto.claim_token),
            claim_expires_at: dto.claim_expires_at,
        })
    }
}
impl TryFrom<v1::DtoApiClaim> for crate::runs::ApiClaim {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiClaim) -> Result<Self, Self::Error> {
        let result: Self = crate::runs::ApiClaim {
            task: (required(wire.task, "task")?).try_into()?,
            run: (required(wire.run, "run")?).try_into()?,
            claim_token: required(wire.claim_token, "claim_token")?,
            claim_expires_at: wire.claim_expires_at,
        };
        Ok(result)
    }
}
impl TryFrom<crate::comments::ApiComment> for v1::DtoApiComment {
    type Error = RpcCodecError;
    fn try_from(dto: crate::comments::ApiComment) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            task_id: Some(dto.task_id),
            author: Some(dto.author),
            author_type: Some(i32::from(v1::DtoCommentsCommentAuthorType::try_from(
                dto.author_type,
            )?)),
            agent_type: dto.agent_type,
            body: Some(dto.body),
            kind: Some(i32::from(v1::DtoCommentsCommentKind::try_from(dto.kind)?)),
            metadata: Some((dto.metadata).try_into()?),
            created_at: Some(dto.created_at),
        })
    }
}
impl TryFrom<v1::DtoApiComment> for crate::comments::ApiComment {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiComment) -> Result<Self, Self::Error> {
        let result: Self = crate::comments::ApiComment {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            task_id: required(wire.task_id, "task_id")?,
            author: required(wire.author, "author")?,
            author_type: v1::DtoCommentsCommentAuthorType::try_from(required(
                wire.author_type,
                "author_type",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            agent_type: wire.agent_type,
            body: required(wire.body, "body")?,
            kind: v1::DtoCommentsCommentKind::try_from(required(wire.kind, "kind")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            metadata: (required(wire.metadata, "metadata")?).try_into()?,
            created_at: required(wire.created_at, "created_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::create_task::ApiCreateTaskStatus> for v1::DtoApiCreateTaskStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::create_task::ApiCreateTaskStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::create_task::ApiCreateTaskStatus::Triage => Self::Triage,
            crate::create_task::ApiCreateTaskStatus::Todo => Self::Todo,
            crate::create_task::ApiCreateTaskStatus::Scheduled => Self::Scheduled,
            crate::create_task::ApiCreateTaskStatus::Ready => Self::Ready,
        })
    }
}
impl TryFrom<v1::DtoApiCreateTaskStatus> for crate::create_task::ApiCreateTaskStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiCreateTaskStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiCreateTaskStatus::Triage => Self::Triage,
            v1::DtoApiCreateTaskStatus::Todo => Self::Todo,
            v1::DtoApiCreateTaskStatus::Scheduled => Self::Scheduled,
            v1::DtoApiCreateTaskStatus::Ready => Self::Ready,
            v1::DtoApiCreateTaskStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::dependencies::ApiDependencies> for v1::DtoApiDependencies {
    type Error = RpcCodecError;
    fn try_from(dto: crate::dependencies::ApiDependencies) -> Result<Self, Self::Error> {
        Ok(Self {
            task: Some((dto.task).try_into()?),
            parents: (dto.parents)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            children: (dto.children)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            edges: (dto.edges)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoApiDependencies> for crate::dependencies::ApiDependencies {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiDependencies) -> Result<Self, Self::Error> {
        let result: Self = crate::dependencies::ApiDependencies {
            task: (required(wire.task, "task")?).try_into()?,
            parents: (wire.parents)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            children: (wire.children)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            edges: (wire.edges)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::dependencies::ApiDependencyEdge> for v1::DtoApiDependencyEdge {
    type Error = RpcCodecError;
    fn try_from(dto: crate::dependencies::ApiDependencyEdge) -> Result<Self, Self::Error> {
        Ok(Self {
            parent: Some((dto.parent).try_into()?),
            child: Some((dto.child).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoApiDependencyEdge> for crate::dependencies::ApiDependencyEdge {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiDependencyEdge) -> Result<Self, Self::Error> {
        let result: Self = crate::dependencies::ApiDependencyEdge {
            parent: (required(wire.parent, "parent")?).try_into()?,
            child: (required(wire.child, "child")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::dependencies::ApiDependencyTask> for v1::DtoApiDependencyTask {
    type Error = RpcCodecError;
    fn try_from(dto: crate::dependencies::ApiDependencyTask) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            board_slug: Some(dto.board_slug),
            task_ref: Some(dto.task_ref),
            title: Some(dto.title),
            status: Some(i32::from(v1::DtoApiTaskStatus::try_from(dto.status)?)),
        })
    }
}
impl TryFrom<v1::DtoApiDependencyTask> for crate::dependencies::ApiDependencyTask {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiDependencyTask) -> Result<Self, Self::Error> {
        let result: Self = crate::dependencies::ApiDependencyTask {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            board_slug: required(wire.board_slug, "board_slug")?,
            task_ref: required(wire.task_ref, "task_ref")?,
            title: required(wire.title, "title")?,
            status: v1::DtoApiTaskStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::ApiErrorCode> for v1::DtoApiErrorCode {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::ApiErrorCode) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::wire::ApiErrorCode::NotFound => Self::NotFound,
            crate::wire::ApiErrorCode::Conflict => Self::Conflict,
            crate::wire::ApiErrorCode::IdempotencyConflict => Self::IdempotencyConflict,
            crate::wire::ApiErrorCode::DependencyCycle => Self::DependencyCycle,
            crate::wire::ApiErrorCode::InvalidInput => Self::InvalidInput,
            crate::wire::ApiErrorCode::FeatureNotAvailable => Self::FeatureNotAvailable,
            crate::wire::ApiErrorCode::ServerUnavailable => Self::ServerUnavailable,
            crate::wire::ApiErrorCode::ExecutionPlanRequired => Self::ExecutionPlanRequired,
            crate::wire::ApiErrorCode::StepsIncomplete => Self::StepsIncomplete,
            crate::wire::ApiErrorCode::ClaimTokenMismatch => Self::ClaimTokenMismatch,
            crate::wire::ApiErrorCode::DependencyBlocked => Self::DependencyBlocked,
            crate::wire::ApiErrorCode::ClaimConflict => Self::ClaimConflict,
            crate::wire::ApiErrorCode::InvalidTransition => Self::InvalidTransition,
            crate::wire::ApiErrorCode::Internal => Self::Internal,
        })
    }
}
impl TryFrom<v1::DtoApiErrorCode> for crate::wire::ApiErrorCode {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiErrorCode) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiErrorCode::NotFound => Self::NotFound,
            v1::DtoApiErrorCode::Conflict => Self::Conflict,
            v1::DtoApiErrorCode::IdempotencyConflict => Self::IdempotencyConflict,
            v1::DtoApiErrorCode::DependencyCycle => Self::DependencyCycle,
            v1::DtoApiErrorCode::InvalidInput => Self::InvalidInput,
            v1::DtoApiErrorCode::FeatureNotAvailable => Self::FeatureNotAvailable,
            v1::DtoApiErrorCode::ServerUnavailable => Self::ServerUnavailable,
            v1::DtoApiErrorCode::ExecutionPlanRequired => Self::ExecutionPlanRequired,
            v1::DtoApiErrorCode::StepsIncomplete => Self::StepsIncomplete,
            v1::DtoApiErrorCode::ClaimTokenMismatch => Self::ClaimTokenMismatch,
            v1::DtoApiErrorCode::DependencyBlocked => Self::DependencyBlocked,
            v1::DtoApiErrorCode::ClaimConflict => Self::ClaimConflict,
            v1::DtoApiErrorCode::InvalidTransition => Self::InvalidTransition,
            v1::DtoApiErrorCode::Internal => Self::Internal,
            v1::DtoApiErrorCode::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::steps::ApiExecutionPlan> for v1::DtoApiExecutionPlan {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::ApiExecutionPlan) -> Result<Self, Self::Error> {
        Ok(Self {
            board_id: Some(dto.board_id),
            task_id: Some(dto.task_id),
            state: Some(i32::from(v1::DtoApiExecutionPlanState::try_from(
                dto.state,
            )?)),
            reason: dto.reason,
            updated_by: Some(dto.updated_by),
            updated_at: Some(dto.updated_at),
        })
    }
}
impl TryFrom<v1::DtoApiExecutionPlan> for crate::steps::ApiExecutionPlan {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiExecutionPlan) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::ApiExecutionPlan {
            board_id: required(wire.board_id, "board_id")?,
            task_id: required(wire.task_id, "task_id")?,
            state: v1::DtoApiExecutionPlanState::try_from(required(wire.state, "state")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            reason: wire.reason,
            updated_by: required(wire.updated_by, "updated_by")?,
            updated_at: required(wire.updated_at, "updated_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::api_components::ApiExecutionPlanState> for v1::DtoApiExecutionPlanState {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ApiExecutionPlanState) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::api_components::ApiExecutionPlanState::Unplanned => Self::Unplanned,
            crate::api_components::ApiExecutionPlanState::Planned => Self::Planned,
            crate::api_components::ApiExecutionPlanState::NotRequired => Self::NotRequired,
        })
    }
}
impl TryFrom<v1::DtoApiExecutionPlanState> for crate::api_components::ApiExecutionPlanState {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiExecutionPlanState) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiExecutionPlanState::Unplanned => Self::Unplanned,
            v1::DtoApiExecutionPlanState::Planned => Self::Planned,
            v1::DtoApiExecutionPlanState::NotRequired => Self::NotRequired,
            v1::DtoApiExecutionPlanState::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::api_components::ApiLabel> for v1::DtoApiLabel {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ApiLabel) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            name: Some(dto.name),
            color: dto.color,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
        })
    }
}
impl TryFrom<v1::DtoApiLabel> for crate::api_components::ApiLabel {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiLabel) -> Result<Self, Self::Error> {
        let result: Self = crate::api_components::ApiLabel {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            name: required(wire.name, "name")?,
            color: wire.color,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::ApiRelation> for v1::DtoApiRelation {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ApiRelation) -> Result<Self, Self::Error> {
        Ok(Self {
            subject_uri: Some(dto.subject_uri),
            predicate: Some(dto.predicate),
            object_uri: Some(dto.object_uri),
            graph_uri: Some(dto.graph_uri),
            provenance: Some((dto.provenance).try_into()?),
            metadata: Some(encode_json(dto.metadata)?),
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
        })
    }
}
impl TryFrom<v1::DtoApiRelation> for crate::derived::ApiRelation {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiRelation) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ApiRelation {
            subject_uri: required(wire.subject_uri, "subject_uri")?,
            predicate: required(wire.predicate, "predicate")?,
            object_uri: required(wire.object_uri, "object_uri")?,
            graph_uri: required(wire.graph_uri, "graph_uri")?,
            provenance: (required(wire.provenance, "provenance")?).try_into()?,
            metadata: decode_json(required(wire.metadata, "metadata")?)?,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::ApiRelationProvenance> for v1::DtoApiRelationProvenance {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ApiRelationProvenance) -> Result<Self, Self::Error> {
        Ok(Self {
            source_table: dto.source_table,
            source_id: dto.source_id,
            source_event_id: dto.source_event_id,
            authoritative_store: Some(dto.authoritative_store),
        })
    }
}
impl TryFrom<v1::DtoApiRelationProvenance> for crate::derived::ApiRelationProvenance {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiRelationProvenance) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ApiRelationProvenance {
            source_table: wire.source_table,
            source_id: wire.source_id,
            source_event_id: wire.source_event_id,
            authoritative_store: required(wire.authoritative_store, "authoritative_store")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::runs::ApiRun> for v1::DtoApiRun {
    type Error = RpcCodecError;
    fn try_from(dto: crate::runs::ApiRun) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            task_id: Some(dto.task_id),
            status: Some(i32::from(v1::DtoApiRunStatus::try_from(dto.status)?)),
            worker_profile: dto.worker_profile,
            worker_pid: dto.worker_pid,
            claim_owner: Some(dto.claim_owner),
            started_at: Some(dto.started_at),
            finished_at: dto.finished_at,
            exit_code: dto.exit_code,
            summary: dto.summary,
            error: dto.error,
            has_log: Some(dto.has_log),
            metadata: Some(encode_json(dto.metadata)?),
        })
    }
}
impl TryFrom<v1::DtoApiRun> for crate::runs::ApiRun {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiRun) -> Result<Self, Self::Error> {
        let result: Self = crate::runs::ApiRun {
            id: required(wire.id, "id")?,
            task_id: required(wire.task_id, "task_id")?,
            status: v1::DtoApiRunStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            worker_profile: wire.worker_profile,
            worker_pid: wire.worker_pid,
            claim_owner: required(wire.claim_owner, "claim_owner")?,
            started_at: required(wire.started_at, "started_at")?,
            finished_at: wire.finished_at,
            exit_code: wire.exit_code,
            summary: wire.summary,
            error: wire.error,
            has_log: required(wire.has_log, "has_log")?,
            metadata: decode_json(required(wire.metadata, "metadata")?)?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::runs::ApiRunLog> for v1::DtoApiRunLog {
    type Error = RpcCodecError;
    fn try_from(dto: crate::runs::ApiRunLog) -> Result<Self, Self::Error> {
        Ok(Self {
            run_id: Some(dto.run_id),
            content: Some(dto.content),
            truncated: Some(dto.truncated),
        })
    }
}
impl TryFrom<v1::DtoApiRunLog> for crate::runs::ApiRunLog {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiRunLog) -> Result<Self, Self::Error> {
        let result: Self = crate::runs::ApiRunLog {
            run_id: required(wire.run_id, "run_id")?,
            content: required(wire.content, "content")?,
            truncated: required(wire.truncated, "truncated")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::runs::ApiRunStatus> for v1::DtoApiRunStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::runs::ApiRunStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::runs::ApiRunStatus::Running => Self::Running,
            crate::runs::ApiRunStatus::Succeeded => Self::Succeeded,
            crate::runs::ApiRunStatus::Failed => Self::Failed,
            crate::runs::ApiRunStatus::Canceled => Self::Canceled,
            crate::runs::ApiRunStatus::Expired => Self::Expired,
        })
    }
}
impl TryFrom<v1::DtoApiRunStatus> for crate::runs::ApiRunStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiRunStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiRunStatus::Running => Self::Running,
            v1::DtoApiRunStatus::Succeeded => Self::Succeeded,
            v1::DtoApiRunStatus::Failed => Self::Failed,
            v1::DtoApiRunStatus::Canceled => Self::Canceled,
            v1::DtoApiRunStatus::Expired => Self::Expired,
            v1::DtoApiRunStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::steps::ApiStepStatus> for v1::DtoApiStepStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::ApiStepStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::steps::ApiStepStatus::Todo => Self::Todo,
            crate::steps::ApiStepStatus::Done => Self::Done,
            crate::steps::ApiStepStatus::Skipped => Self::Skipped,
        })
    }
}
impl TryFrom<v1::DtoApiStepStatus> for crate::steps::ApiStepStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiStepStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiStepStatus::Todo => Self::Todo,
            v1::DtoApiStepStatus::Done => Self::Done,
            v1::DtoApiStepStatus::Skipped => Self::Skipped,
            v1::DtoApiStepStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::api_components::ApiTask> for v1::DtoApiTask {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ApiTask) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            board_slug: Some(dto.board_slug),
            task_ref: Some(dto.task_ref),
            seq: Some(dto.seq),
            title: Some(dto.title),
            description: dto.description,
            status: Some(i32::from(v1::DtoApiTaskStatus::try_from(dto.status)?)),
            status_reason: dto.status_reason,
            assignee: dto.assignee,
            priority: Some((dto.priority).try_into()?),
            position: Some(dto.position),
            scheduled_at: dto.scheduled_at,
            due_at: dto.due_at,
            created_by: Some(dto.created_by),
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            started_at: dto.started_at,
            completed_at: dto.completed_at,
            archived_at: dto.archived_at,
            claim_owner: dto.claim_owner,
            claim_expires_at: dto.claim_expires_at,
            last_heartbeat_at: dto.last_heartbeat_at,
            current_run_id: dto.current_run_id,
            retry_count: Some(dto.retry_count),
            max_retries: dto.max_retries,
            result_summary: dto.result_summary,
            result: dto
                .result
                .map(|value| -> Result<_, RpcCodecError> { encode_json(value) })
                .transpose()?,
            metadata: Some(encode_json(dto.metadata)?),
            lock_version: Some(dto.lock_version),
            dependency_blocked: Some(dto.dependency_blocked),
            unfinished_parent_count: Some(dto.unfinished_parent_count),
            execution_plan_state: Some(i32::from(v1::DtoApiExecutionPlanState::try_from(
                dto.execution_plan_state,
            )?)),
            required_step_count: Some(dto.required_step_count),
            completed_required_step_count: Some(dto.completed_required_step_count),
            optional_step_count: Some(dto.optional_step_count),
            labels: (dto.labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoApiTask> for crate::api_components::ApiTask {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiTask) -> Result<Self, Self::Error> {
        let result: Self = crate::api_components::ApiTask {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            board_slug: required(wire.board_slug, "board_slug")?,
            task_ref: required(wire.task_ref, "task_ref")?,
            seq: required(wire.seq, "seq")?,
            title: required(wire.title, "title")?,
            description: wire.description,
            status: v1::DtoApiTaskStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            status_reason: wire.status_reason,
            assignee: wire.assignee,
            priority: (required(wire.priority, "priority")?).try_into()?,
            position: required(wire.position, "position")?,
            scheduled_at: wire.scheduled_at,
            due_at: wire.due_at,
            created_by: required(wire.created_by, "created_by")?,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            started_at: wire.started_at,
            completed_at: wire.completed_at,
            archived_at: wire.archived_at,
            claim_owner: wire.claim_owner,
            claim_expires_at: wire.claim_expires_at,
            last_heartbeat_at: wire.last_heartbeat_at,
            current_run_id: wire.current_run_id,
            retry_count: required(wire.retry_count, "retry_count")?,
            max_retries: wire.max_retries,
            result_summary: wire.result_summary,
            result: wire
                .result
                .map(|value| -> Result<_, RpcCodecError> { decode_json(value) })
                .transpose()?,
            metadata: decode_json(required(wire.metadata, "metadata")?)?,
            lock_version: required(wire.lock_version, "lock_version")?,
            dependency_blocked: required(wire.dependency_blocked, "dependency_blocked")?,
            unfinished_parent_count: required(
                wire.unfinished_parent_count,
                "unfinished_parent_count",
            )?,
            execution_plan_state: v1::DtoApiExecutionPlanState::try_from(required(
                wire.execution_plan_state,
                "execution_plan_state",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            required_step_count: required(wire.required_step_count, "required_step_count")?,
            completed_required_step_count: required(
                wire.completed_required_step_count,
                "completed_required_step_count",
            )?,
            optional_step_count: required(wire.optional_step_count, "optional_step_count")?,
            labels: (wire.labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_graph::ApiTaskGraphEdgeKind> for v1::DtoApiTaskGraphEdgeKind {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_graph::ApiTaskGraphEdgeKind) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::task_graph::ApiTaskGraphEdgeKind::Dependency => Self::Dependency,
            crate::task_graph::ApiTaskGraphEdgeKind::Step => Self::Step,
        })
    }
}
impl TryFrom<v1::DtoApiTaskGraphEdgeKind> for crate::task_graph::ApiTaskGraphEdgeKind {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiTaskGraphEdgeKind) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiTaskGraphEdgeKind::Dependency => Self::Dependency,
            v1::DtoApiTaskGraphEdgeKind::Step => Self::Step,
            v1::DtoApiTaskGraphEdgeKind::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::task_graph::ApiTaskGraphNodeRole> for v1::DtoApiTaskGraphNodeRole {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_graph::ApiTaskGraphNodeRole) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::task_graph::ApiTaskGraphNodeRole::Center => Self::Center,
            crate::task_graph::ApiTaskGraphNodeRole::DependencyParent => Self::DependencyParent,
            crate::task_graph::ApiTaskGraphNodeRole::DependencyChild => Self::DependencyChild,
            crate::task_graph::ApiTaskGraphNodeRole::StepParent => Self::StepParent,
            crate::task_graph::ApiTaskGraphNodeRole::StepChild => Self::StepChild,
            crate::task_graph::ApiTaskGraphNodeRole::Active => Self::Active,
            crate::task_graph::ApiTaskGraphNodeRole::Context => Self::Context,
        })
    }
}
impl TryFrom<v1::DtoApiTaskGraphNodeRole> for crate::task_graph::ApiTaskGraphNodeRole {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiTaskGraphNodeRole) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiTaskGraphNodeRole::Center => Self::Center,
            v1::DtoApiTaskGraphNodeRole::DependencyParent => Self::DependencyParent,
            v1::DtoApiTaskGraphNodeRole::DependencyChild => Self::DependencyChild,
            v1::DtoApiTaskGraphNodeRole::StepParent => Self::StepParent,
            v1::DtoApiTaskGraphNodeRole::StepChild => Self::StepChild,
            v1::DtoApiTaskGraphNodeRole::Active => Self::Active,
            v1::DtoApiTaskGraphNodeRole::Context => Self::Context,
            v1::DtoApiTaskGraphNodeRole::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::api_components::ApiTaskPriority> for v1::DtoApiTaskPriority {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ApiTaskPriority) -> Result<Self, Self::Error> {
        Ok(Self {
            value: Some(u32::from(dto.get())),
        })
    }
}
impl TryFrom<v1::DtoApiTaskPriority> for crate::api_components::ApiTaskPriority {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiTaskPriority) -> Result<Self, Self::Error> {
        crate::api_components::ApiTaskPriority::new(
            u8::try_from(required(wire.value, "value")?)
                .map_err(|_| RpcCodecError::invalid("uint32 超出 u8"))?,
        )
        .ok_or_else(|| RpcCodecError::invalid("ApiTaskPriority"))
    }
}
impl TryFrom<crate::api_components::ApiTaskStatus> for v1::DtoApiTaskStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ApiTaskStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::api_components::ApiTaskStatus::Triage => Self::Triage,
            crate::api_components::ApiTaskStatus::Todo => Self::Todo,
            crate::api_components::ApiTaskStatus::Scheduled => Self::Scheduled,
            crate::api_components::ApiTaskStatus::Ready => Self::Ready,
            crate::api_components::ApiTaskStatus::Running => Self::Running,
            crate::api_components::ApiTaskStatus::Blocked => Self::Blocked,
            crate::api_components::ApiTaskStatus::Review => Self::Review,
            crate::api_components::ApiTaskStatus::Done => Self::Done,
            crate::api_components::ApiTaskStatus::Archived => Self::Archived,
        })
    }
}
impl TryFrom<v1::DtoApiTaskStatus> for crate::api_components::ApiTaskStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiTaskStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoApiTaskStatus::Triage => Self::Triage,
            v1::DtoApiTaskStatus::Todo => Self::Todo,
            v1::DtoApiTaskStatus::Scheduled => Self::Scheduled,
            v1::DtoApiTaskStatus::Ready => Self::Ready,
            v1::DtoApiTaskStatus::Running => Self::Running,
            v1::DtoApiTaskStatus::Blocked => Self::Blocked,
            v1::DtoApiTaskStatus::Review => Self::Review,
            v1::DtoApiTaskStatus::Done => Self::Done,
            v1::DtoApiTaskStatus::Archived => Self::Archived,
            v1::DtoApiTaskStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::steps::ApiTaskStep> for v1::DtoApiTaskStep {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::ApiTaskStep) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            parent_task_id: Some(dto.parent_task_id),
            title: Some(dto.title),
            body: dto.body,
            linked_task: dto
                .linked_task
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            position: Some(dto.position),
            required: Some(dto.required),
            status: Some(i32::from(v1::DtoApiStepStatus::try_from(dto.status)?)),
            resolution_note: dto.resolution_note,
            resolved_by: dto.resolved_by,
            resolved_at: dto.resolved_at,
            created_by: Some(dto.created_by),
            created_at: Some(dto.created_at),
            updated_by: Some(dto.updated_by),
            updated_at: Some(dto.updated_at),
        })
    }
}
impl TryFrom<v1::DtoApiTaskStep> for crate::steps::ApiTaskStep {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiTaskStep) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::ApiTaskStep {
            id: required(wire.id, "id")?,
            parent_task_id: required(wire.parent_task_id, "parent_task_id")?,
            title: required(wire.title, "title")?,
            body: wire.body,
            linked_task: wire
                .linked_task
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            position: required(wire.position, "position")?,
            required: required(wire.required, "required")?,
            status: v1::DtoApiStepStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            resolution_note: wire.resolution_note,
            resolved_by: wire.resolved_by,
            resolved_at: wire.resolved_at,
            created_by: required(wire.created_by, "created_by")?,
            created_at: required(wire.created_at, "created_at")?,
            updated_by: required(wire.updated_by, "updated_by")?,
            updated_at: required(wire.updated_at, "updated_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::ApiTaskSteps> for v1::DtoApiTaskSteps {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::ApiTaskSteps) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: Some(dto.task_id),
            steps: (dto.steps)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            execution_plan: Some((dto.execution_plan).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoApiTaskSteps> for crate::steps::ApiTaskSteps {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoApiTaskSteps) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::ApiTaskSteps {
            task_id: required(wire.task_id, "task_id")?,
            steps: (wire.steps)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            execution_plan: (required(wire.execution_plan, "execution_plan")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::BackupReport> for v1::DtoBackupReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::BackupReport) -> Result<Self, Self::Error> {
        Ok(Self {
            out_path: Some(dto.out_path),
            checksum_sha256: Some(dto.checksum_sha256),
            bytes: Some(dto.bytes),
            source_fingerprint: Some(dto.source_fingerprint),
        })
    }
}
impl TryFrom<v1::DtoBackupReport> for crate::maintenance::BackupReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoBackupReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::BackupReport {
            out_path: required(wire.out_path, "out_path")?,
            checksum_sha256: required(wire.checksum_sha256, "checksum_sha256")?,
            bytes: required(wire.bytes, "bytes")?,
            source_fingerprint: required(wire.source_fingerprint, "source_fingerprint")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::BlockedReasonCount> for v1::DtoBlockedReasonCount {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::BlockedReasonCount) -> Result<Self, Self::Error> {
        Ok(Self {
            reason: Some(dto.reason),
            count: Some(dto.count),
        })
    }
}
impl TryFrom<v1::DtoBlockedReasonCount> for crate::derived::BlockedReasonCount {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoBlockedReasonCount) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::BlockedReasonCount {
            reason: required(wire.reason, "reason")?,
            count: required(wire.count, "count")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::BoardCreatedPayload> for v1::DtoBoardCreatedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::BoardCreatedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            slug: Some(dto.slug),
        })
    }
}
impl TryFrom<v1::DtoBoardCreatedPayload> for crate::event_payload::BoardCreatedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoBoardCreatedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::BoardCreatedPayload {
            slug: required(wire.slug, "slug")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_graph::BoardTaskMap> for v1::DtoBoardTaskMap {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_graph::BoardTaskMap) -> Result<Self, Self::Error> {
        Ok(Self {
            nodes: (dto.nodes)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            edges: (dto.edges)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoBoardTaskMap> for crate::task_graph::BoardTaskMap {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoBoardTaskMap) -> Result<Self, Self::Error> {
        let result: Self = crate::task_graph::BoardTaskMap {
            nodes: (wire.nodes)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            edges: (wire.edges)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::BootstrapTaskLabelData> for v1::DtoBootstrapTaskLabelData {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::BootstrapTaskLabelData) -> Result<Self, Self::Error> {
        Ok(Self {
            task: Some((dto.task).try_into()?),
            semantics: Some((dto.semantics).try_into()?),
            verification: dto
                .verification
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::DtoBootstrapTaskLabelData> for crate::label_surfaces::BootstrapTaskLabelData {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoBootstrapTaskLabelData) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::BootstrapTaskLabelData {
            task: (required(wire.task, "task")?).try_into()?,
            semantics: (required(wire.semantics, "semantics")?).try_into()?,
            verification: wire
                .verification
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::BootstrapTaskLabelVerification>
    for v1::DtoBootstrapTaskLabelVerification
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::BootstrapTaskLabelVerification,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            label_name: Some(dto.label_name),
            score: Some(finite(dto.score)?),
            source: Some(dto.source),
            min_score: Some(finite(dto.min_score)?),
            degraded: Some(dto.degraded),
            diagnostics: dto.diagnostics,
        })
    }
}
impl TryFrom<v1::DtoBootstrapTaskLabelVerification>
    for crate::label_surfaces::BootstrapTaskLabelVerification
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoBootstrapTaskLabelVerification) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::BootstrapTaskLabelVerification {
            label_name: required(wire.label_name, "label_name")?,
            score: finite(required(wire.score, "score")?)?,
            source: required(wire.source, "source")?,
            min_score: finite(required(wire.min_score, "min_score")?)?,
            degraded: required(wire.degraded, "degraded")?,
            diagnostics: wire.diagnostics,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::CheckpointReport> for v1::DtoCheckpointReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::CheckpointReport) -> Result<Self, Self::Error> {
        Ok(Self {
            busy: Some(dto.busy),
            log_frames: Some(dto.log_frames),
            checkpointed_frames: Some(dto.checkpointed_frames),
        })
    }
}
impl TryFrom<v1::DtoCheckpointReport> for crate::maintenance::CheckpointReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCheckpointReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::CheckpointReport {
            busy: required(wire.busy, "busy")?,
            log_frames: required(wire.log_frames, "log_frames")?,
            checkpointed_frames: required(wire.checkpointed_frames, "checkpointed_frames")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli::CliEntity> for v1::DtoCliEntity {
    type Error = RpcCodecError;
    fn try_from(dto: crate::cli::CliEntity) -> Result<Self, Self::Error> {
        Ok(Self {
            uri: Some(dto.uri),
            kind: Some(dto.kind),
            source_table: Some(dto.source_table),
            source_id: Some(dto.source_id),
            board_id: dto.board_id,
            task_id: dto.task_id,
            title: dto.title,
            summary: dto.summary,
            content_hash: dto.content_hash,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            archived_at: dto.archived_at,
        })
    }
}
impl TryFrom<v1::DtoCliEntity> for crate::cli::CliEntity {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliEntity) -> Result<Self, Self::Error> {
        let result: Self = crate::cli::CliEntity {
            uri: required(wire.uri, "uri")?,
            kind: required(wire.kind, "kind")?,
            source_table: required(wire.source_table, "source_table")?,
            source_id: required(wire.source_id, "source_id")?,
            board_id: wire.board_id,
            task_id: wire.task_id,
            title: wire.title,
            summary: wire.summary,
            content_hash: wire.content_hash,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            archived_at: wire.archived_at,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli_helpers::CliGraphQueryBinding> for v1::DtoCliGraphQueryBinding {
    type Error = RpcCodecError;
    fn try_from(dto: crate::cli_helpers::CliGraphQueryBinding) -> Result<Self, Self::Error> {
        Ok(Self {
            name: Some(dto.name),
            value: Some(dto.value),
        })
    }
}
impl TryFrom<v1::DtoCliGraphQueryBinding> for crate::cli_helpers::CliGraphQueryBinding {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliGraphQueryBinding) -> Result<Self, Self::Error> {
        let result: Self = crate::cli_helpers::CliGraphQueryBinding {
            name: required(wire.name, "name")?,
            value: required(wire.value, "value")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli_helpers::CliGraphQueryRow> for v1::DtoCliGraphQueryRow {
    type Error = RpcCodecError;
    fn try_from(dto: crate::cli_helpers::CliGraphQueryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            bindings: (dto.bindings)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoCliGraphQueryRow> for crate::cli_helpers::CliGraphQueryRow {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliGraphQueryRow) -> Result<Self, Self::Error> {
        let result: Self = crate::cli_helpers::CliGraphQueryRow {
            bindings: (wire.bindings)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli_labels::CliLabelOntologyPrecisionRecall>
    for v1::DtoCliLabelOntologyPrecisionRecall
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::cli_labels::CliLabelOntologyPrecisionRecall,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            available: Some(dto.available),
            reason: Some(dto.reason),
        })
    }
}
impl TryFrom<v1::DtoCliLabelOntologyPrecisionRecall>
    for crate::cli_labels::CliLabelOntologyPrecisionRecall
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliLabelOntologyPrecisionRecall) -> Result<Self, Self::Error> {
        let result: Self = crate::cli_labels::CliLabelOntologyPrecisionRecall {
            available: required(wire.available, "available")?,
            reason: required(wire.reason, "reason")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli_labels::CliLabelOntologyQuality> for v1::DtoCliLabelOntologyQuality {
    type Error = RpcCodecError;
    fn try_from(dto: crate::cli_labels::CliLabelOntologyQuality) -> Result<Self, Self::Error> {
        Ok(Self {
            board_id: Some(dto.board_id),
            denominator: Some((dto.denominator).try_into()?),
            disagreement: Some((dto.disagreement).try_into()?),
            rates: Some((dto.rates).try_into()?),
            precision_recall: Some((dto.precision_recall).try_into()?),
            warnings: dto.warnings,
        })
    }
}
impl TryFrom<v1::DtoCliLabelOntologyQuality> for crate::cli_labels::CliLabelOntologyQuality {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliLabelOntologyQuality) -> Result<Self, Self::Error> {
        let result: Self = crate::cli_labels::CliLabelOntologyQuality {
            board_id: required(wire.board_id, "board_id")?,
            denominator: (required(wire.denominator, "denominator")?).try_into()?,
            disagreement: (required(wire.disagreement, "disagreement")?).try_into()?,
            rates: (required(wire.rates, "rates")?).try_into()?,
            precision_recall: (required(wire.precision_recall, "precision_recall")?).try_into()?,
            warnings: wire.warnings,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli_labels::CliLabelOntologyQualityDenominator>
    for v1::DtoCliLabelOntologyQualityDenominator
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::cli_labels::CliLabelOntologyQualityDenominator,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            source: Some(dto.source),
            description: Some(dto.description),
            observation_count: Some(dto.observation_count),
            distinct_task_count: Some(dto.distinct_task_count),
            agreement_observation_count: Some(dto.agreement_observation_count),
            agreement_task_count: Some(dto.agreement_task_count),
            degraded_observation_count: Some(dto.degraded_observation_count),
            first_observed_at: dto.first_observed_at,
            latest_observed_at: dto.latest_observed_at,
            sample_task_refs: dto.sample_task_refs,
        })
    }
}
impl TryFrom<v1::DtoCliLabelOntologyQualityDenominator>
    for crate::cli_labels::CliLabelOntologyQualityDenominator
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliLabelOntologyQualityDenominator) -> Result<Self, Self::Error> {
        let result: Self = crate::cli_labels::CliLabelOntologyQualityDenominator {
            source: required(wire.source, "source")?,
            description: required(wire.description, "description")?,
            observation_count: required(wire.observation_count, "observation_count")?,
            distinct_task_count: required(wire.distinct_task_count, "distinct_task_count")?,
            agreement_observation_count: required(
                wire.agreement_observation_count,
                "agreement_observation_count",
            )?,
            agreement_task_count: required(wire.agreement_task_count, "agreement_task_count")?,
            degraded_observation_count: required(
                wire.degraded_observation_count,
                "degraded_observation_count",
            )?,
            first_observed_at: wire.first_observed_at,
            latest_observed_at: wire.latest_observed_at,
            sample_task_refs: wire.sample_task_refs,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli_labels::CliLabelOntologyQualityDisagreement>
    for v1::DtoCliLabelOntologyQualityDisagreement
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::cli_labels::CliLabelOntologyQualityDisagreement,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            signal_count: Some(dto.signal_count),
            distinct_task_count: Some(dto.distinct_task_count),
            by_kind: (dto.by_kind)
                .into_iter()
                .map(|(key, value)| -> Result<_, RpcCodecError> { Ok((key, value)) })
                .collect::<Result<_, _>>()?,
            by_status: (dto.by_status)
                .into_iter()
                .map(|(key, value)| -> Result<_, RpcCodecError> { Ok((key, value)) })
                .collect::<Result<_, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoCliLabelOntologyQualityDisagreement>
    for crate::cli_labels::CliLabelOntologyQualityDisagreement
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliLabelOntologyQualityDisagreement) -> Result<Self, Self::Error> {
        let result: Self = crate::cli_labels::CliLabelOntologyQualityDisagreement {
            signal_count: required(wire.signal_count, "signal_count")?,
            distinct_task_count: required(wire.distinct_task_count, "distinct_task_count")?,
            by_kind: (wire.by_kind)
                .into_iter()
                .map(|(key, value)| -> Result<_, RpcCodecError> { Ok((key, value)) })
                .collect::<Result<_, _>>()?,
            by_status: (wire.by_status)
                .into_iter()
                .map(|(key, value)| -> Result<_, RpcCodecError> { Ok((key, value)) })
                .collect::<Result<_, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::cli_labels::CliLabelOntologyQualityRates>
    for v1::DtoCliLabelOntologyQualityRates
{
    type Error = RpcCodecError;
    fn try_from(dto: crate::cli_labels::CliLabelOntologyQualityRates) -> Result<Self, Self::Error> {
        Ok(Self {
            disagreement_task_rate: dto
                .disagreement_task_rate
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            disagreement_task_rate_basis: Some(dto.disagreement_task_rate_basis),
        })
    }
}
impl TryFrom<v1::DtoCliLabelOntologyQualityRates>
    for crate::cli_labels::CliLabelOntologyQualityRates
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCliLabelOntologyQualityRates) -> Result<Self, Self::Error> {
        let result: Self = crate::cli_labels::CliLabelOntologyQualityRates {
            disagreement_task_rate: wire
                .disagreement_task_rate
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            disagreement_task_rate_basis: required(
                wire.disagreement_task_rate_basis,
                "disagreement_task_rate_basis",
            )?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::comments::CommentAuthorType> for v1::DtoCommentsCommentAuthorType {
    type Error = RpcCodecError;
    fn try_from(dto: crate::comments::CommentAuthorType) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::comments::CommentAuthorType::User => Self::User,
            crate::comments::CommentAuthorType::Agent => Self::Agent,
        })
    }
}
impl TryFrom<v1::DtoCommentsCommentAuthorType> for crate::comments::CommentAuthorType {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCommentsCommentAuthorType) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoCommentsCommentAuthorType::User => Self::User,
            v1::DtoCommentsCommentAuthorType::Agent => Self::Agent,
            v1::DtoCommentsCommentAuthorType::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::comments::CommentKind> for v1::DtoCommentsCommentKind {
    type Error = RpcCodecError;
    fn try_from(dto: crate::comments::CommentKind) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::comments::CommentKind::Note => Self::Note,
            crate::comments::CommentKind::Decision => Self::Decision,
            crate::comments::CommentKind::Signal => Self::Signal,
        })
    }
}
impl TryFrom<v1::DtoCommentsCommentKind> for crate::comments::CommentKind {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCommentsCommentKind) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoCommentsCommentKind::Note => Self::Note,
            v1::DtoCommentsCommentKind::Decision => Self::Decision,
            v1::DtoCommentsCommentKind::Signal => Self::Signal,
            v1::DtoCommentsCommentKind::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::derived::ContextDiagnostic> for v1::DtoContextDiagnostic {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ContextDiagnostic) -> Result<Self, Self::Error> {
        Ok(Self {
            source: Some(dto.source),
            code: Some(dto.code),
            message: Some(dto.message),
        })
    }
}
impl TryFrom<v1::DtoContextDiagnostic> for crate::derived::ContextDiagnostic {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoContextDiagnostic) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ContextDiagnostic {
            source: required(wire.source, "source")?,
            code: required(wire.code, "code")?,
            message: required(wire.message, "message")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::ContextEvidence> for v1::DtoContextEvidence {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ContextEvidence) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: Some(dto.kind),
            entity_uri: dto.entity_uri,
            task_id: dto.task_id,
            relation_id: dto.relation_id,
            predicate: dto.predicate,
            summary: dto.summary,
        })
    }
}
impl TryFrom<v1::DtoContextEvidence> for crate::derived::ContextEvidence {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoContextEvidence) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ContextEvidence {
            kind: required(wire.kind, "kind")?,
            entity_uri: wire.entity_uri,
            task_id: wire.task_id,
            relation_id: wire.relation_id,
            predicate: wire.predicate,
            summary: wire.summary,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::ContextItem> for v1::DtoContextItem {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ContextItem) -> Result<Self, Self::Error> {
        Ok(Self {
            entity_uri: Some(dto.entity_uri),
            source: Some(dto.source),
            provenance: dto.provenance,
            score: dto
                .score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            title: dto.title,
            snippet: dto.snippet,
            rank: Some(
                u64::try_from(dto.rank).map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            reason: Some(dto.reason),
            evidence: (dto.evidence)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoContextItem> for crate::derived::ContextItem {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoContextItem) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ContextItem {
            entity_uri: required(wire.entity_uri, "entity_uri")?,
            source: required(wire.source, "source")?,
            provenance: wire.provenance,
            score: wire
                .score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            title: wire.title,
            snippet: wire.snippet,
            rank: match wire.rank {
                Some(value) => usize::try_from(value)
                    .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                None => Default::default(),
            },
            reason: wire.reason.unwrap_or_default(),
            evidence: (wire.evidence)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::ContextPack> for v1::DtoContextPack {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ContextPack) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: Some(dto.subject),
            policy: Some((dto.policy).try_into()?),
            items: (dto.items)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            degraded: dto.degraded,
            diagnostics: (dto.diagnostics)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            providers: (dto.providers)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            truncated: Some(dto.truncated),
            truncation_reason: dto.truncation_reason,
        })
    }
}
impl TryFrom<v1::DtoContextPack> for crate::derived::ContextPack {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoContextPack) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ContextPack {
            subject: required(wire.subject, "subject")?,
            policy: (required(wire.policy, "policy")?).try_into()?,
            items: (wire.items)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            degraded: wire.degraded,
            diagnostics: (wire.diagnostics)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            providers: (wire.providers)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            truncated: wire.truncated.unwrap_or_default(),
            truncation_reason: wire.truncation_reason,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::ContextPolicy> for v1::DtoContextPolicy {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ContextPolicy) -> Result<Self, Self::Error> {
        Ok(Self {
            depth: Some(
                u64::try_from(dto.depth)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            lexical_limit: Some(
                u64::try_from(dto.lexical_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            graph_limit: Some(
                u64::try_from(dto.graph_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            vector_limit: Some(
                u64::try_from(dto.vector_limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            max_items: Some(
                u64::try_from(dto.max_items)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            budget: dto
                .budget
                .map(|value| -> Result<_, RpcCodecError> {
                    u64::try_from(value).map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))
                })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::DtoContextPolicy> for crate::derived::ContextPolicy {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoContextPolicy) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ContextPolicy {
            depth: match wire.depth {
                Some(value) => usize::try_from(value)
                    .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
                None => 1,
            },
            lexical_limit: usize::try_from(required(wire.lexical_limit, "lexical_limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            graph_limit: usize::try_from(required(wire.graph_limit, "graph_limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            vector_limit: usize::try_from(required(wire.vector_limit, "vector_limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            max_items: usize::try_from(required(wire.max_items, "max_items")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            budget: wire
                .budget
                .map(|value| -> Result<_, RpcCodecError> {
                    usize::try_from(value).map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))
                })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::ContextProviderStatus> for v1::DtoContextProviderStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::ContextProviderStatus) -> Result<Self, Self::Error> {
        Ok(Self {
            provider: Some(dto.provider),
            capability: Some(dto.capability),
            available: Some(dto.available),
            degraded: Some(dto.degraded),
            reason: dto.reason,
        })
    }
}
impl TryFrom<v1::DtoContextProviderStatus> for crate::derived::ContextProviderStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoContextProviderStatus) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::ContextProviderStatus {
            provider: required(wire.provider, "provider")?,
            capability: required(wire.capability, "capability")?,
            available: required(wire.available, "available")?,
            degraded: required(wire.degraded, "degraded")?,
            reason: wire.reason,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::CreatedLabelsMeta<crate::api_components::ApiLabel>>
    for v1::DtoCreatedLabelsMetaOfDtoApiLabel
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::CreatedLabelsMeta<crate::api_components::ApiLabel>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            created_labels: (dto.created_labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoCreatedLabelsMetaOfDtoApiLabel>
    for crate::wire::CreatedLabelsMeta<crate::api_components::ApiLabel>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoCreatedLabelsMetaOfDtoApiLabel) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::CreatedLabelsMeta {
            created_labels: (wire.created_labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::DeleteBoardLabelResult> for v1::DtoDeleteBoardLabelResult {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::DeleteBoardLabelResult) -> Result<Self, Self::Error> {
        Ok(Self {
            label: Some((dto.label).try_into()?),
            forced: Some(dto.forced),
            removed_task_bindings: Some(dto.removed_task_bindings),
            removed_semantics: Some(dto.removed_semantics),
            removed_atoms: Some(dto.removed_atoms),
        })
    }
}
impl TryFrom<v1::DtoDeleteBoardLabelResult> for crate::label_surfaces::DeleteBoardLabelResult {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoDeleteBoardLabelResult) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::DeleteBoardLabelResult {
            label: (required(wire.label, "label")?).try_into()?,
            forced: required(wire.forced, "forced")?,
            removed_task_bindings: required(wire.removed_task_bindings, "removed_task_bindings")?,
            removed_semantics: required(wire.removed_semantics, "removed_semantics")?,
            removed_atoms: required(wire.removed_atoms, "removed_atoms")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DeleteResult> for v1::DtoDeleteResult {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::DeleteResult) -> Result<Self, Self::Error> {
        Ok(Self {
            deleted: Some(dto.deleted),
        })
    }
}
impl TryFrom<v1::DtoDeleteResult> for crate::wire::DeleteResult {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoDeleteResult) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DeleteResult {
            deleted: required(wire.deleted, "deleted")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::DependencyPayload> for v1::DtoDependencyPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::DependencyPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            parent_task_id: Some(dto.parent_task_id),
        })
    }
}
impl TryFrom<v1::DtoDependencyPayload> for crate::event_payload::DependencyPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoDependencyPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::DependencyPayload {
            parent_task_id: required(wire.parent_task_id, "parent_task_id")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::DoctorDerivedStore> for v1::DtoDoctorDerivedStore {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::DoctorDerivedStore) -> Result<Self, Self::Error> {
        Ok(Self {
            store_name: Some(dto.store_name),
            schema_version: Some(dto.schema_version),
            last_event_id: Some(dto.last_event_id),
            dirty: Some(dto.dirty),
            last_error: dto.last_error,
            pending_outbox: Some(dto.pending_outbox),
            running_outbox: Some(dto.running_outbox),
            failed_outbox: Some(dto.failed_outbox),
        })
    }
}
impl TryFrom<v1::DtoDoctorDerivedStore> for crate::maintenance::DoctorDerivedStore {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoDoctorDerivedStore) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::DoctorDerivedStore {
            store_name: required(wire.store_name, "store_name")?,
            schema_version: required(wire.schema_version, "schema_version")?,
            last_event_id: required(wire.last_event_id, "last_event_id")?,
            dirty: required(wire.dirty, "dirty")?,
            last_error: wire.last_error,
            pending_outbox: required(wire.pending_outbox, "pending_outbox")?,
            running_outbox: required(wire.running_outbox, "running_outbox")?,
            failed_outbox: required(wire.failed_outbox, "failed_outbox")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::DoctorIssue> for v1::DtoDoctorIssue {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::DoctorIssue) -> Result<Self, Self::Error> {
        Ok(Self {
            severity: Some(dto.severity),
            code: Some(dto.code),
            message: Some(dto.message),
            record_ids: dto.record_ids,
        })
    }
}
impl TryFrom<v1::DtoDoctorIssue> for crate::maintenance::DoctorIssue {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoDoctorIssue) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::DoctorIssue {
            severity: required(wire.severity, "severity")?,
            code: required(wire.code, "code")?,
            message: required(wire.message, "message")?,
            record_ids: wire.record_ids,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::DoctorReport> for v1::DtoDoctorReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::DoctorReport) -> Result<Self, Self::Error> {
        Ok(Self {
            ok: Some(dto.ok),
            integrity_check: Some(dto.integrity_check),
            migration_version: dto.migration_version,
            user_version: Some(dto.user_version),
            expired_running_tasks: Some(dto.expired_running_tasks),
            running_tasks_without_active_run: Some(dto.running_tasks_without_active_run),
            orphan_running_runs: Some(dto.orphan_running_runs),
            dependency_cycles: Some(dto.dependency_cycles),
            archived_dependency_edges: Some(dto.archived_dependency_edges),
            missing_run_logs: Some(dto.missing_run_logs),
            suspicious_run_log_paths: Some(dto.suspicious_run_log_paths),
            executable_dependency_violations: Some(dto.executable_dependency_violations),
            executable_spec_violations: Some(dto.executable_spec_violations),
            executable_schedule_violations: Some(dto.executable_schedule_violations),
            unplanned_active_tasks: Some(dto.unplanned_active_tasks),
            active_parents_with_incomplete_required_steps: Some(
                dto.active_parents_with_incomplete_required_steps,
            ),
            outbox_pending: Some(dto.outbox_pending),
            outbox_running: Some(dto.outbox_running),
            outbox_failed: Some(dto.outbox_failed),
            derived_dirty_stores: Some(dto.derived_dirty_stores),
            derived_error_stores: Some(dto.derived_error_stores),
            derived_stores: (dto.derived_stores)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            consistency_errors: Some(dto.consistency_errors),
            consistency_warnings: Some(dto.consistency_warnings),
            consistency_issues: (dto.consistency_issues)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            ontology_ledger_errors: Some(dto.ontology_ledger_errors),
            ontology_ledger_warnings: Some(dto.ontology_ledger_warnings),
            ontology_ledger_issues: (dto.ontology_ledger_issues)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoDoctorReport> for crate::maintenance::DoctorReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoDoctorReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::DoctorReport {
            ok: required(wire.ok, "ok")?,
            integrity_check: required(wire.integrity_check, "integrity_check")?,
            migration_version: wire.migration_version,
            user_version: required(wire.user_version, "user_version")?,
            expired_running_tasks: required(wire.expired_running_tasks, "expired_running_tasks")?,
            running_tasks_without_active_run: required(
                wire.running_tasks_without_active_run,
                "running_tasks_without_active_run",
            )?,
            orphan_running_runs: required(wire.orphan_running_runs, "orphan_running_runs")?,
            dependency_cycles: required(wire.dependency_cycles, "dependency_cycles")?,
            archived_dependency_edges: required(
                wire.archived_dependency_edges,
                "archived_dependency_edges",
            )?,
            missing_run_logs: required(wire.missing_run_logs, "missing_run_logs")?,
            suspicious_run_log_paths: required(
                wire.suspicious_run_log_paths,
                "suspicious_run_log_paths",
            )?,
            executable_dependency_violations: required(
                wire.executable_dependency_violations,
                "executable_dependency_violations",
            )?,
            executable_spec_violations: required(
                wire.executable_spec_violations,
                "executable_spec_violations",
            )?,
            executable_schedule_violations: required(
                wire.executable_schedule_violations,
                "executable_schedule_violations",
            )?,
            unplanned_active_tasks: required(
                wire.unplanned_active_tasks,
                "unplanned_active_tasks",
            )?,
            active_parents_with_incomplete_required_steps: required(
                wire.active_parents_with_incomplete_required_steps,
                "active_parents_with_incomplete_required_steps",
            )?,
            outbox_pending: required(wire.outbox_pending, "outbox_pending")?,
            outbox_running: required(wire.outbox_running, "outbox_running")?,
            outbox_failed: required(wire.outbox_failed, "outbox_failed")?,
            derived_dirty_stores: required(wire.derived_dirty_stores, "derived_dirty_stores")?,
            derived_error_stores: required(wire.derived_error_stores, "derived_error_stores")?,
            derived_stores: (wire.derived_stores)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            consistency_errors: required(wire.consistency_errors, "consistency_errors")?,
            consistency_warnings: required(wire.consistency_warnings, "consistency_warnings")?,
            consistency_issues: (wire.consistency_issues)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            ontology_ledger_errors: required(
                wire.ontology_ledger_errors,
                "ontology_ledger_errors",
            )?,
            ontology_ledger_warnings: required(
                wire.ontology_ledger_warnings,
                "ontology_ledger_warnings",
            )?,
            ontology_ledger_issues: (wire.ontology_ledger_issues)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::EmptyPayload> for v1::DtoEmptyPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::EmptyPayload) -> Result<Self, Self::Error> {
        let _ = dto;
        Ok(Self {})
    }
}
impl TryFrom<v1::DtoEmptyPayload> for crate::event_payload::EmptyPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoEmptyPayload) -> Result<Self, Self::Error> {
        let _ = wire;
        let result: Self = crate::event_payload::EmptyPayload {};
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::EventPayload> for v1::DtoEventPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::EventPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            value: Some(match dto {
                crate::event_payload::EventPayload::Empty(value) => {
                    v1::dto_event_payload::Value::Empty((value).try_into()?)
                }
                crate::event_payload::EventPayload::BoardCreated(value) => {
                    v1::dto_event_payload::Value::BoardCreated((value).try_into()?)
                }
                crate::event_payload::EventPayload::Dependency(value) => {
                    v1::dto_event_payload::Value::Dependency((value).try_into()?)
                }
                crate::event_payload::EventPayload::LabelCreated(value) => {
                    v1::dto_event_payload::Value::LabelCreated((value).try_into()?)
                }
                crate::event_payload::EventPayload::LabelDeleted(value) => {
                    v1::dto_event_payload::Value::LabelDeleted((value).try_into()?)
                }
                crate::event_payload::EventPayload::LabelOntologyObservationRecorded(value) => {
                    v1::dto_event_payload::Value::LabelOntologyObservationRecorded(
                        (value).try_into()?,
                    )
                }
                crate::event_payload::EventPayload::LabelOntologyActionCreated(value) => {
                    v1::dto_event_payload::Value::LabelOntologyActionCreated((value).try_into()?)
                }
                crate::event_payload::EventPayload::LabelOntologySignalReviewed(value) => {
                    v1::dto_event_payload::Value::LabelOntologySignalReviewed((value).try_into()?)
                }
                crate::event_payload::EventPayload::SignalRecorded(value) => {
                    v1::dto_event_payload::Value::SignalRecorded((value).try_into()?)
                }
                crate::event_payload::EventPayload::SignalReviewed(value) => {
                    v1::dto_event_payload::Value::SignalReviewed((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskReason(value) => {
                    v1::dto_event_payload::Value::TaskReason((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskClaimed(value) => {
                    v1::dto_event_payload::Value::TaskClaimed((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskCommentCreated(value) => {
                    v1::dto_event_payload::Value::TaskCommentCreated((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskResult(value) => {
                    v1::dto_event_payload::Value::TaskResult((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskStatus(value) => {
                    v1::dto_event_payload::Value::TaskStatus((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskToStatus(value) => {
                    v1::dto_event_payload::Value::TaskToStatus((value).try_into()?)
                }
                crate::event_payload::EventPayload::ExecutionPlan(value) => {
                    v1::dto_event_payload::Value::ExecutionPlan((value).try_into()?)
                }
                crate::event_payload::EventPayload::Heartbeat(value) => {
                    v1::dto_event_payload::Value::Heartbeat((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskLabel(value) => {
                    v1::dto_event_payload::Value::TaskLabel((value).try_into()?)
                }
                crate::event_payload::EventPayload::LabelProposal(value) => {
                    v1::dto_event_payload::Value::LabelProposal((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskReclaimed(value) => {
                    v1::dto_event_payload::Value::TaskReclaimed((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskRetry(value) => {
                    v1::dto_event_payload::Value::TaskRetry((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskReopened(value) => {
                    v1::dto_event_payload::Value::TaskReopened((value).try_into()?)
                }
                crate::event_payload::EventPayload::RetryPolicy(value) => {
                    v1::dto_event_payload::Value::RetryPolicy((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskStep(value) => {
                    v1::dto_event_payload::Value::TaskStep((value).try_into()?)
                }
                crate::event_payload::EventPayload::TaskExportSanitized(value) => {
                    v1::dto_event_payload::Value::TaskExportSanitized((value).try_into()?)
                }
                crate::event_payload::EventPayload::Unknown(value) => {
                    v1::dto_event_payload::Value::Unknown(encode_json(value)?)
                }
            }),
        })
    }
}
impl TryFrom<v1::DtoEventPayload> for crate::event_payload::EventPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoEventPayload) -> Result<Self, Self::Error> {
        Ok(match required(wire.value, "DtoEventPayload")? {
            v1::dto_event_payload::Value::Empty(value) => Self::Empty((value).try_into()?),
            v1::dto_event_payload::Value::BoardCreated(value) => {
                Self::BoardCreated((value).try_into()?)
            }
            v1::dto_event_payload::Value::Dependency(value) => {
                Self::Dependency((value).try_into()?)
            }
            v1::dto_event_payload::Value::LabelCreated(value) => {
                Self::LabelCreated((value).try_into()?)
            }
            v1::dto_event_payload::Value::LabelDeleted(value) => {
                Self::LabelDeleted((value).try_into()?)
            }
            v1::dto_event_payload::Value::LabelOntologyObservationRecorded(value) => {
                Self::LabelOntologyObservationRecorded((value).try_into()?)
            }
            v1::dto_event_payload::Value::LabelOntologyActionCreated(value) => {
                Self::LabelOntologyActionCreated((value).try_into()?)
            }
            v1::dto_event_payload::Value::LabelOntologySignalReviewed(value) => {
                Self::LabelOntologySignalReviewed((value).try_into()?)
            }
            v1::dto_event_payload::Value::SignalRecorded(value) => {
                Self::SignalRecorded((value).try_into()?)
            }
            v1::dto_event_payload::Value::SignalReviewed(value) => {
                Self::SignalReviewed((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskReason(value) => {
                Self::TaskReason((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskClaimed(value) => {
                Self::TaskClaimed((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskCommentCreated(value) => {
                Self::TaskCommentCreated((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskResult(value) => {
                Self::TaskResult((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskStatus(value) => {
                Self::TaskStatus((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskToStatus(value) => {
                Self::TaskToStatus((value).try_into()?)
            }
            v1::dto_event_payload::Value::ExecutionPlan(value) => {
                Self::ExecutionPlan((value).try_into()?)
            }
            v1::dto_event_payload::Value::Heartbeat(value) => Self::Heartbeat((value).try_into()?),
            v1::dto_event_payload::Value::TaskLabel(value) => Self::TaskLabel((value).try_into()?),
            v1::dto_event_payload::Value::LabelProposal(value) => {
                Self::LabelProposal((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskReclaimed(value) => {
                Self::TaskReclaimed((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskRetry(value) => Self::TaskRetry((value).try_into()?),
            v1::dto_event_payload::Value::TaskReopened(value) => {
                Self::TaskReopened((value).try_into()?)
            }
            v1::dto_event_payload::Value::RetryPolicy(value) => {
                Self::RetryPolicy((value).try_into()?)
            }
            v1::dto_event_payload::Value::TaskStep(value) => Self::TaskStep((value).try_into()?),
            v1::dto_event_payload::Value::TaskExportSanitized(value) => {
                Self::TaskExportSanitized((value).try_into()?)
            }
            v1::dto_event_payload::Value::Unknown(value) => Self::Unknown(decode_json(value)?),
        })
    }
}
impl TryFrom<crate::event_payload::CommentAuthorType> for v1::DtoEventPayloadCommentAuthorType {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::CommentAuthorType) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::CommentAuthorType::User => Self::User,
            crate::event_payload::CommentAuthorType::Agent => Self::Agent,
        })
    }
}
impl TryFrom<v1::DtoEventPayloadCommentAuthorType> for crate::event_payload::CommentAuthorType {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoEventPayloadCommentAuthorType) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoEventPayloadCommentAuthorType::User => Self::User,
            v1::DtoEventPayloadCommentAuthorType::Agent => Self::Agent,
            v1::DtoEventPayloadCommentAuthorType::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::event_payload::CommentKind> for v1::DtoEventPayloadCommentKind {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::CommentKind) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::CommentKind::Note => Self::Note,
            crate::event_payload::CommentKind::Decision => Self::Decision,
            crate::event_payload::CommentKind::Signal => Self::Signal,
        })
    }
}
impl TryFrom<v1::DtoEventPayloadCommentKind> for crate::event_payload::CommentKind {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoEventPayloadCommentKind) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoEventPayloadCommentKind::Note => Self::Note,
            v1::DtoEventPayloadCommentKind::Decision => Self::Decision,
            v1::DtoEventPayloadCommentKind::Signal => Self::Signal,
            v1::DtoEventPayloadCommentKind::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::event_payload::ExecutionPlanPayload> for v1::DtoExecutionPlanPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::ExecutionPlanPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            state: Some(i32::from(v1::DtoExecutionPlanState::try_from(dto.state)?)),
        })
    }
}
impl TryFrom<v1::DtoExecutionPlanPayload> for crate::event_payload::ExecutionPlanPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoExecutionPlanPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::ExecutionPlanPayload {
            state: v1::DtoExecutionPlanState::try_from(required(wire.state, "state")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::ExecutionPlanState> for v1::DtoExecutionPlanState {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::ExecutionPlanState) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::ExecutionPlanState::Planned => Self::Planned,
            crate::event_payload::ExecutionPlanState::NotRequired => Self::NotRequired,
            crate::event_payload::ExecutionPlanState::Unplanned => Self::Unplanned,
        })
    }
}
impl TryFrom<v1::DtoExecutionPlanState> for crate::event_payload::ExecutionPlanState {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoExecutionPlanState) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoExecutionPlanState::Planned => Self::Planned,
            v1::DtoExecutionPlanState::NotRequired => Self::NotRequired,
            v1::DtoExecutionPlanState::Unplanned => Self::Unplanned,
            v1::DtoExecutionPlanState::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::maintenance::ExportReport> for v1::DtoExportReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::ExportReport) -> Result<Self, Self::Error> {
        Ok(Self {
            out_path: Some(dto.out_path),
            checksum_sha256: Some(dto.checksum_sha256),
            bytes: Some(dto.bytes),
            record_count: Some(dto.record_count),
            source_fingerprint: Some(dto.source_fingerprint),
        })
    }
}
impl TryFrom<v1::DtoExportReport> for crate::maintenance::ExportReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoExportReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::ExportReport {
            out_path: required(wire.out_path, "out_path")?,
            checksum_sha256: required(wire.checksum_sha256, "checksum_sha256")?,
            bytes: required(wire.bytes, "bytes")?,
            record_count: required(wire.record_count, "record_count")?,
            source_fingerprint: required(wire.source_fingerprint, "source_fingerprint")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::GraphMaintenance> for v1::DtoGraphMaintenance {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::GraphMaintenance) -> Result<Self, Self::Error> {
        Ok(Self {
            mode: Some(dto.mode),
            board_id: Some(dto.board_id),
            generation: Some(dto.generation),
            fingerprint: Some(dto.fingerprint),
            validated_tasks: Some(dto.validated_tasks),
            validated_entities: Some(dto.validated_entities),
            validated_relations: Some(dto.validated_relations),
            pending_jobs: Some(dto.pending_jobs),
            consumed_jobs: Some(dto.consumed_jobs),
            updated_at: Some(dto.updated_at),
            message: Some(dto.message),
        })
    }
}
impl TryFrom<v1::DtoGraphMaintenance> for crate::derived::GraphMaintenance {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoGraphMaintenance) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::GraphMaintenance {
            mode: required(wire.mode, "mode")?,
            board_id: required(wire.board_id, "board_id")?,
            generation: required(wire.generation, "generation")?,
            fingerprint: required(wire.fingerprint, "fingerprint")?,
            validated_tasks: required(wire.validated_tasks, "validated_tasks")?,
            validated_entities: required(wire.validated_entities, "validated_entities")?,
            validated_relations: required(wire.validated_relations, "validated_relations")?,
            pending_jobs: required(wire.pending_jobs, "pending_jobs")?,
            consumed_jobs: required(wire.consumed_jobs, "consumed_jobs")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            message: required(wire.message, "message")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::GraphStatus> for v1::DtoGraphStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::GraphStatus) -> Result<Self, Self::Error> {
        Ok(Self {
            backend: Some(dto.backend),
            enabled: Some(dto.enabled),
            message: Some(dto.message),
        })
    }
}
impl TryFrom<v1::DtoGraphStatus> for crate::derived::GraphStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoGraphStatus) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::GraphStatus {
            backend: required(wire.backend, "backend")?,
            enabled: required(wire.enabled, "enabled")?,
            message: required(wire.message, "message")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::HealthReport> for v1::DtoHealthReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::HealthReport) -> Result<Self, Self::Error> {
        Ok(Self {
            ok: Some(dto.ok),
            db: Some(dto.db),
            version: Some(dto.version),
            db_path: Some(dto.db_path),
            db_fingerprint: Some(dto.db_fingerprint),
        })
    }
}
impl TryFrom<v1::DtoHealthReport> for crate::wire::HealthReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoHealthReport) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::HealthReport {
            ok: required(wire.ok, "ok")?,
            db: required(wire.db, "db")?,
            version: required(wire.version, "version")?,
            db_path: required(wire.db_path, "db_path")?,
            db_fingerprint: required(wire.db_fingerprint, "db_fingerprint")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::HeartbeatPayload> for v1::DtoHeartbeatPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::HeartbeatPayload) -> Result<Self, Self::Error> {
        Ok(Self { note: dto.note })
    }
}
impl TryFrom<v1::DtoHeartbeatPayload> for crate::event_payload::HeartbeatPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoHeartbeatPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::HeartbeatPayload { note: wire.note };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::ImportReport> for v1::DtoImportReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::ImportReport) -> Result<Self, Self::Error> {
        Ok(Self {
            in_path: Some(dto.in_path),
            source_fingerprint: Some(dto.source_fingerprint),
            imported_records: Some(dto.imported_records),
            skipped_records: Some(dto.skipped_records),
            rebuild_jobs_enqueued: Some(dto.rebuild_jobs_enqueued),
            journal_id: Some(dto.journal_id),
            phase: Some(dto.phase),
            restart_required: Some(dto.restart_required),
            staged_database_path: dto.staged_database_path,
            target_fingerprint_before: dto.target_fingerprint_before,
            staged_fingerprint: dto.staged_fingerprint,
            publish_preconditions: dto.publish_preconditions,
        })
    }
}
impl TryFrom<v1::DtoImportReport> for crate::maintenance::ImportReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoImportReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::ImportReport {
            in_path: required(wire.in_path, "in_path")?,
            source_fingerprint: required(wire.source_fingerprint, "source_fingerprint")?,
            imported_records: required(wire.imported_records, "imported_records")?,
            skipped_records: required(wire.skipped_records, "skipped_records")?,
            rebuild_jobs_enqueued: required(wire.rebuild_jobs_enqueued, "rebuild_jobs_enqueued")?,
            journal_id: required(wire.journal_id, "journal_id")?,
            phase: required(wire.phase, "phase")?,
            restart_required: required(wire.restart_required, "restart_required")?,
            staged_database_path: wire.staged_database_path,
            target_fingerprint_before: wire.target_fingerprint_before,
            staged_fingerprint: wire.staged_fingerprint,
            publish_preconditions: wire.publish_preconditions,
        };
        Ok(result)
    }
}
impl TryFrom<crate::structured_metadata::JsonArray> for v1::DtoJsonArray {
    type Error = RpcCodecError;
    fn try_from(dto: crate::structured_metadata::JsonArray) -> Result<Self, Self::Error> {
        Ok(Self {
            value: (dto.0)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { encode_json(value) })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoJsonArray> for crate::structured_metadata::JsonArray {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoJsonArray) -> Result<Self, Self::Error> {
        Ok(crate::structured_metadata::JsonArray(
            (wire.value)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { decode_json(value) })
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
impl TryFrom<crate::label_surfaces::LabelAtomExplainActionWire>
    for v1::DtoLabelAtomExplainActionWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelAtomExplainActionWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            action: Some((dto.action).try_into()?),
            matched_by: Some(dto.matched_by),
        })
    }
}
impl TryFrom<v1::DtoLabelAtomExplainActionWire>
    for crate::label_surfaces::LabelAtomExplainActionWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelAtomExplainActionWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelAtomExplainActionWire {
            action: (required(wire.action, "action")?).try_into()?,
            matched_by: required(wire.matched_by, "matched_by")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelAtomExplainSignalWire>
    for v1::DtoLabelAtomExplainSignalWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelAtomExplainSignalWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            signal: Some((dto.signal).try_into()?),
            observation: Some((dto.observation).try_into()?),
            source_task: Some((dto.source_task).try_into()?),
            task_ref_snapshot: Some(dto.task_ref_snapshot),
            suggest_input_stale: Some(dto.suggest_input_stale),
            suggest_degraded: Some(dto.suggest_degraded),
            warnings: dto.warnings,
        })
    }
}
impl TryFrom<v1::DtoLabelAtomExplainSignalWire>
    for crate::label_surfaces::LabelAtomExplainSignalWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelAtomExplainSignalWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelAtomExplainSignalWire {
            signal: (required(wire.signal, "signal")?).try_into()?,
            observation: (required(wire.observation, "observation")?).try_into()?,
            source_task: (required(wire.source_task, "source_task")?).try_into()?,
            task_ref_snapshot: required(wire.task_ref_snapshot, "task_ref_snapshot")?,
            suggest_input_stale: required(wire.suggest_input_stale, "suggest_input_stale")?,
            suggest_degraded: required(wire.suggest_degraded, "suggest_degraded")?,
            warnings: wire.warnings,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelAtomExplainValidationWire>
    for v1::DtoLabelAtomExplainValidationWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelAtomExplainValidationWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            action: Some((dto.action).try_into()?),
            parent_action_id: Some(dto.parent_action_id),
            validation_status: Some(i32::from(
                v1::DtoLabelOntologyValidationStatusWire::try_from(dto.validation_status)?,
            )),
            manual: Some(encode_json(dto.manual)?),
            summary: Some(encode_json(dto.summary)?),
            cases: Some(encode_json(dto.cases)?),
            warnings: dto.warnings,
        })
    }
}
impl TryFrom<v1::DtoLabelAtomExplainValidationWire>
    for crate::label_surfaces::LabelAtomExplainValidationWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelAtomExplainValidationWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelAtomExplainValidationWire {
            action: (required(wire.action, "action")?).try_into()?,
            parent_action_id: required(wire.parent_action_id, "parent_action_id")?,
            validation_status: v1::DtoLabelOntologyValidationStatusWire::try_from(required(
                wire.validation_status,
                "validation_status",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            manual: decode_json(required(wire.manual, "manual")?)?,
            summary: decode_json(required(wire.summary, "summary")?)?,
            cases: decode_json(required(wire.cases, "cases")?)?,
            warnings: wire.warnings,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelAtomExplainWire> for v1::DtoLabelAtomExplainWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::LabelAtomExplainWire) -> Result<Self, Self::Error> {
        Ok(Self {
            query: Some(dto.query),
            atom: dto
                .atom
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            current_semantics: dto
                .current_semantics
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            provenance_actions: (dto.provenance_actions)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            supporting_signals: (dto.supporting_signals)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            validation_history: (dto.validation_history)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            legacy_untracked: Some(dto.legacy_untracked),
            legacy_reason: dto.legacy_reason,
        })
    }
}
impl TryFrom<v1::DtoLabelAtomExplainWire> for crate::label_surfaces::LabelAtomExplainWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelAtomExplainWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelAtomExplainWire {
            query: required(wire.query, "query")?,
            atom: wire
                .atom
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            current_semantics: wire
                .current_semantics
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            provenance_actions: (wire.provenance_actions)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            supporting_signals: (wire.supporting_signals)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            validation_history: (wire.validation_history)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            legacy_untracked: required(wire.legacy_untracked, "legacy_untracked")?,
            legacy_reason: wire.legacy_reason,
        };
        Ok(result)
    }
}
impl TryFrom<crate::rpc::dto::LabelAtomIndexHit> for v1::DtoLabelAtomIndexHit {
    type Error = RpcCodecError;
    fn try_from(dto: crate::rpc::dto::LabelAtomIndexHit) -> Result<Self, Self::Error> {
        Ok(Self {
            atom_id: Some(dto.atom_id),
            label_id: Some(dto.label_id),
            label_name: Some(dto.label_name),
            board_id: Some(dto.board_id),
            polarity: Some(dto.polarity),
            kind: Some(dto.kind),
            text: Some(dto.text),
            ordinal: Some(dto.ordinal),
            content_hash: Some(dto.content_hash),
            embedding_model: Some(dto.embedding_model),
            distance: Some(finite(dto.distance)?),
        })
    }
}
impl TryFrom<v1::DtoLabelAtomIndexHit> for crate::rpc::dto::LabelAtomIndexHit {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelAtomIndexHit) -> Result<Self, Self::Error> {
        let result: Self = crate::rpc::dto::LabelAtomIndexHit {
            atom_id: required(wire.atom_id, "atom_id")?,
            label_id: required(wire.label_id, "label_id")?,
            label_name: required(wire.label_name, "label_name")?,
            board_id: required(wire.board_id, "board_id")?,
            polarity: required(wire.polarity, "polarity")?,
            kind: required(wire.kind, "kind")?,
            text: required(wire.text, "text")?,
            ordinal: required(wire.ordinal, "ordinal")?,
            content_hash: required(wire.content_hash, "content_hash")?,
            embedding_model: required(wire.embedding_model, "embedding_model")?,
            distance: finite(required(wire.distance, "distance")?)?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::rpc::dto::LabelAtomIndexQueryData> for v1::DtoLabelAtomIndexQueryData {
    type Error = RpcCodecError;
    fn try_from(dto: crate::rpc::dto::LabelAtomIndexQueryData) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            degraded: Some(dto.degraded),
            diagnostics: dto.diagnostics,
        })
    }
}
impl TryFrom<v1::DtoLabelAtomIndexQueryData> for crate::rpc::dto::LabelAtomIndexQueryData {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelAtomIndexQueryData) -> Result<Self, Self::Error> {
        let result: Self = crate::rpc::dto::LabelAtomIndexQueryData {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            degraded: required(wire.degraded, "degraded")?,
            diagnostics: wire.diagnostics,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelAtomWire> for v1::DtoLabelAtomWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::LabelAtomWire) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            label_id: Some(dto.label_id),
            board_id: Some(dto.board_id),
            label_name: Some(dto.label_name),
            polarity: Some(dto.polarity),
            kind: Some(dto.kind),
            text: Some(dto.text),
            ordinal: Some(dto.ordinal),
            content_hash: Some(dto.content_hash),
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
        })
    }
}
impl TryFrom<v1::DtoLabelAtomWire> for crate::label_surfaces::LabelAtomWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelAtomWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelAtomWire {
            id: required(wire.id, "id")?,
            label_id: required(wire.label_id, "label_id")?,
            board_id: required(wire.board_id, "board_id")?,
            label_name: required(wire.label_name, "label_name")?,
            polarity: required(wire.polarity, "polarity")?,
            kind: required(wire.kind, "kind")?,
            text: required(wire.text, "text")?,
            ordinal: required(wire.ordinal, "ordinal")?,
            content_hash: required(wire.content_hash, "content_hash")?,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::LabelCreatedPayload> for v1::DtoLabelCreatedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::LabelCreatedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            label_id: Some(dto.label_id),
            label: Some(dto.label),
            color: dto.color,
        })
    }
}
impl TryFrom<v1::DtoLabelCreatedPayload> for crate::event_payload::LabelCreatedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelCreatedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::LabelCreatedPayload {
            label_id: required(wire.label_id, "label_id")?,
            label: required(wire.label, "label")?,
            color: wire.color,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::LabelDeletedPayload> for v1::DtoLabelDeletedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::LabelDeletedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            label_id: Some(dto.label_id),
            label: Some(dto.label),
            forced: Some(dto.forced),
            removed_task_bindings: Some(
                u64::try_from(dto.removed_task_bindings)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            removed_semantics: Some(dto.removed_semantics),
            removed_atoms: Some(
                u64::try_from(dto.removed_atoms)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoLabelDeletedPayload> for crate::event_payload::LabelDeletedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelDeletedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::LabelDeletedPayload {
            label_id: required(wire.label_id, "label_id")?,
            label: required(wire.label, "label")?,
            forced: required(wire.forced, "forced")?,
            removed_task_bindings: usize::try_from(required(
                wire.removed_task_bindings,
                "removed_task_bindings",
            )?)
            .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            removed_semantics: required(wire.removed_semantics, "removed_semantics")?,
            removed_atoms: usize::try_from(required(wire.removed_atoms, "removed_atoms")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::LabelOntologyActionCreatedPayload>
    for v1::DtoLabelOntologyActionCreatedPayload
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::event_payload::LabelOntologyActionCreatedPayload,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            action_id: Some(dto.action_id),
            action_type: Some(dto.action_type),
            signal_ids: dto.signal_ids,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyActionCreatedPayload>
    for crate::event_payload::LabelOntologyActionCreatedPayload
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyActionCreatedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::LabelOntologyActionCreatedPayload {
            action_id: required(wire.action_id, "action_id")?,
            action_type: required(wire.action_type, "action_type")?,
            signal_ids: wire.signal_ids,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyActionTypeWire>
    for v1::DtoLabelOntologyActionTypeWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyActionTypeWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologyActionTypeWire::Confirm => Self::Confirm,
            crate::label_surfaces::LabelOntologyActionTypeWire::Reject => Self::Reject,
            crate::label_surfaces::LabelOntologyActionTypeWire::Supersede => Self::Supersede,
            crate::label_surfaces::LabelOntologyActionTypeWire::ResolveNoChange => {
                Self::ResolveNoChange
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::AddPositiveAtom => {
                Self::AddPositiveAtom
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::AddNegativeAtom => {
                Self::AddNegativeAtom
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::AdoptExistingAtom => {
                Self::AdoptExistingAtom
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::UpdateSemantics => {
                Self::UpdateSemantics
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::CreateLabelProposal => {
                Self::CreateLabelProposal
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::BootstrapLabel => {
                Self::BootstrapLabel
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::RenameLabel => Self::RenameLabel,
            crate::label_surfaces::LabelOntologyActionTypeWire::SplitLabel => Self::SplitLabel,
            crate::label_surfaces::LabelOntologyActionTypeWire::MergeLabels => Self::MergeLabels,
            crate::label_surfaces::LabelOntologyActionTypeWire::RevertOntologyMutation => {
                Self::RevertOntologyMutation
            }
            crate::label_surfaces::LabelOntologyActionTypeWire::Validate => Self::Validate,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyActionTypeWire>
    for crate::label_surfaces::LabelOntologyActionTypeWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyActionTypeWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologyActionTypeWire::Confirm => Self::Confirm,
            v1::DtoLabelOntologyActionTypeWire::Reject => Self::Reject,
            v1::DtoLabelOntologyActionTypeWire::Supersede => Self::Supersede,
            v1::DtoLabelOntologyActionTypeWire::ResolveNoChange => Self::ResolveNoChange,
            v1::DtoLabelOntologyActionTypeWire::AddPositiveAtom => Self::AddPositiveAtom,
            v1::DtoLabelOntologyActionTypeWire::AddNegativeAtom => Self::AddNegativeAtom,
            v1::DtoLabelOntologyActionTypeWire::AdoptExistingAtom => Self::AdoptExistingAtom,
            v1::DtoLabelOntologyActionTypeWire::UpdateSemantics => Self::UpdateSemantics,
            v1::DtoLabelOntologyActionTypeWire::CreateLabelProposal => Self::CreateLabelProposal,
            v1::DtoLabelOntologyActionTypeWire::BootstrapLabel => Self::BootstrapLabel,
            v1::DtoLabelOntologyActionTypeWire::RenameLabel => Self::RenameLabel,
            v1::DtoLabelOntologyActionTypeWire::SplitLabel => Self::SplitLabel,
            v1::DtoLabelOntologyActionTypeWire::MergeLabels => Self::MergeLabels,
            v1::DtoLabelOntologyActionTypeWire::RevertOntologyMutation => {
                Self::RevertOntologyMutation
            }
            v1::DtoLabelOntologyActionTypeWire::Validate => Self::Validate,
            v1::DtoLabelOntologyActionTypeWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyActionWire> for v1::DtoLabelOntologyActionWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::LabelOntologyActionWire) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            parent_action_id: dto.parent_action_id,
            action_type: Some(i32::from(v1::DtoLabelOntologyActionTypeWire::try_from(
                dto.action_type,
            )?)),
            reason: Some(dto.reason),
            target_label_id: dto.target_label_id,
            result_label_id: dto.result_label_id,
            result_atom_id: dto.result_atom_id,
            result_atom_content_hash: dto.result_atom_content_hash,
            result_proposal_id: dto.result_proposal_id,
            canonical_before_hash: dto.canonical_before_hash,
            canonical_after_hash: dto.canonical_after_hash,
            change: Some((dto.change).try_into()?),
            validation_requirement: Some(i32::from(
                v1::DtoLabelOntologyValidationRequirementWire::try_from(
                    dto.validation_requirement,
                )?,
            )),
            validation_status: Some(i32::from(
                v1::DtoLabelOntologyValidationStatusWire::try_from(dto.validation_status)?,
            )),
            validation_effective_outcome: Some(i32::from(
                v1::DtoLabelOntologyValidationEffectiveOutcomeWire::try_from(
                    dto.validation_effective_outcome,
                )?,
            )),
            validation_latest_attempt_id: dto.validation_latest_attempt_id,
            validation: Some((dto.validation).try_into()?),
            created_by: Some(dto.created_by),
            created_by_type: Some(dto.created_by_type),
            agent_type: dto.agent_type,
            created_at: Some(dto.created_at),
            signal_ids: dto.signal_ids,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyActionWire> for crate::label_surfaces::LabelOntologyActionWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyActionWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologyActionWire {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            parent_action_id: wire.parent_action_id,
            action_type: v1::DtoLabelOntologyActionTypeWire::try_from(required(
                wire.action_type,
                "action_type",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            reason: required(wire.reason, "reason")?,
            target_label_id: wire.target_label_id,
            result_label_id: wire.result_label_id,
            result_atom_id: wire.result_atom_id,
            result_atom_content_hash: wire.result_atom_content_hash,
            result_proposal_id: wire.result_proposal_id,
            canonical_before_hash: wire.canonical_before_hash,
            canonical_after_hash: wire.canonical_after_hash,
            change: (required(wire.change, "change")?).try_into()?,
            validation_requirement: v1::DtoLabelOntologyValidationRequirementWire::try_from(
                required(wire.validation_requirement, "validation_requirement")?,
            )
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            validation_status: v1::DtoLabelOntologyValidationStatusWire::try_from(required(
                wire.validation_status,
                "validation_status",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            validation_effective_outcome:
                v1::DtoLabelOntologyValidationEffectiveOutcomeWire::try_from(required(
                    wire.validation_effective_outcome,
                    "validation_effective_outcome",
                )?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            validation_latest_attempt_id: wire.validation_latest_attempt_id,
            validation: (required(wire.validation, "validation")?).try_into()?,
            created_by: required(wire.created_by, "created_by")?,
            created_by_type: required(wire.created_by_type, "created_by_type")?,
            agent_type: wire.agent_type,
            created_at: required(wire.created_at, "created_at")?,
            signal_ids: wire.signal_ids,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyActorWire> for v1::DtoLabelOntologyActorWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::LabelOntologyActorWire) -> Result<Self, Self::Error> {
        Ok(Self {
            name: Some(dto.name),
            actor_type: Some(dto.actor_type),
            agent_type: dto.agent_type,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyActorWire> for crate::label_surfaces::LabelOntologyActorWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyActorWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologyActorWire {
            name: required(wire.name, "name")?,
            actor_type: required(wire.actor_type, "actor_type")?,
            agent_type: wire.agent_type,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyCandidateAtomRequest>
    for v1::DtoLabelOntologyCandidateAtomRequest
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyCandidateAtomRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            polarity: Some(dto.polarity),
            kind: Some(dto.kind),
            text: Some(dto.text),
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyCandidateAtomRequest>
    for crate::label_surfaces::LabelOntologyCandidateAtomRequest
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyCandidateAtomRequest) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologyCandidateAtomRequest {
            polarity: required(wire.polarity, "polarity")?,
            kind: required(wire.kind, "kind")?,
            text: required(wire.text, "text")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::LabelOntologyObservationRecordedPayload>
    for v1::DtoLabelOntologyObservationRecordedPayload
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::event_payload::LabelOntologyObservationRecordedPayload,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            observation_id: Some(dto.observation_id),
            signal_ids: dto.signal_ids,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyObservationRecordedPayload>
    for crate::event_payload::LabelOntologyObservationRecordedPayload
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyObservationRecordedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::LabelOntologyObservationRecordedPayload {
            observation_id: required(wire.observation_id, "observation_id")?,
            signal_ids: wire.signal_ids,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyObservationWire>
    for v1::DtoLabelOntologyObservationWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyObservationWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            task_id: Some(dto.task_id),
            task_ref_snapshot: Some(dto.task_ref_snapshot),
            task_snapshot: Some((dto.task_snapshot).try_into()?),
            suggest_input_hash: dto.suggest_input_hash,
            agent_candidates: Some((dto.agent_candidates).try_into()?),
            suggestion_snapshot: Some((dto.suggestion_snapshot).try_into()?),
            final_decision: Some((dto.final_decision).try_into()?),
            suggest_coverage: dto
                .suggest_coverage
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_coverage_cosine: dto
                .suggest_coverage_cosine
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_residual_norm: dto
                .suggest_residual_norm
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_needs_new_label: Some(dto.suggest_needs_new_label),
            suggest_degraded: Some(dto.suggest_degraded),
            diagnostics: Some((dto.diagnostics).try_into()?),
            capture_fingerprint: Some(dto.capture_fingerprint),
            created_by: Some(dto.created_by),
            created_by_type: Some(dto.created_by_type),
            agent_type: dto.agent_type,
            created_at: Some(dto.created_at),
            signals: (dto.signals)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyObservationWire>
    for crate::label_surfaces::LabelOntologyObservationWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyObservationWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologyObservationWire {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            task_id: required(wire.task_id, "task_id")?,
            task_ref_snapshot: required(wire.task_ref_snapshot, "task_ref_snapshot")?,
            task_snapshot: (required(wire.task_snapshot, "task_snapshot")?).try_into()?,
            suggest_input_hash: wire.suggest_input_hash,
            agent_candidates: (required(wire.agent_candidates, "agent_candidates")?).try_into()?,
            suggestion_snapshot: (required(wire.suggestion_snapshot, "suggestion_snapshot")?)
                .try_into()?,
            final_decision: (required(wire.final_decision, "final_decision")?).try_into()?,
            suggest_coverage: wire
                .suggest_coverage
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_coverage_cosine: wire
                .suggest_coverage_cosine
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_residual_norm: wire
                .suggest_residual_norm
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_needs_new_label: required(
                wire.suggest_needs_new_label,
                "suggest_needs_new_label",
            )?,
            suggest_degraded: required(wire.suggest_degraded, "suggest_degraded")?,
            diagnostics: (required(wire.diagnostics, "diagnostics")?).try_into()?,
            capture_fingerprint: required(wire.capture_fingerprint, "capture_fingerprint")?,
            created_by: required(wire.created_by, "created_by")?,
            created_by_type: required(wire.created_by_type, "created_by_type")?,
            agent_type: wire.agent_type,
            created_at: required(wire.created_at, "created_at")?,
            signals: (wire.signals)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyProposedActionWire>
    for v1::DtoLabelOntologyProposedActionWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyProposedActionWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologyProposedActionWire::Observe => Self::Observe,
            crate::label_surfaces::LabelOntologyProposedActionWire::AddPositiveAtom => {
                Self::AddPositiveAtom
            }
            crate::label_surfaces::LabelOntologyProposedActionWire::AddNegativeAtom => {
                Self::AddNegativeAtom
            }
            crate::label_surfaces::LabelOntologyProposedActionWire::UpdateSemantics => {
                Self::UpdateSemantics
            }
            crate::label_surfaces::LabelOntologyProposedActionWire::BootstrapLabel => {
                Self::BootstrapLabel
            }
            crate::label_surfaces::LabelOntologyProposedActionWire::RenameLabel => {
                Self::RenameLabel
            }
            crate::label_surfaces::LabelOntologyProposedActionWire::SplitLabel => Self::SplitLabel,
            crate::label_surfaces::LabelOntologyProposedActionWire::MergeLabels => {
                Self::MergeLabels
            }
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyProposedActionWire>
    for crate::label_surfaces::LabelOntologyProposedActionWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyProposedActionWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologyProposedActionWire::Observe => Self::Observe,
            v1::DtoLabelOntologyProposedActionWire::AddPositiveAtom => Self::AddPositiveAtom,
            v1::DtoLabelOntologyProposedActionWire::AddNegativeAtom => Self::AddNegativeAtom,
            v1::DtoLabelOntologyProposedActionWire::UpdateSemantics => Self::UpdateSemantics,
            v1::DtoLabelOntologyProposedActionWire::BootstrapLabel => Self::BootstrapLabel,
            v1::DtoLabelOntologyProposedActionWire::RenameLabel => Self::RenameLabel,
            v1::DtoLabelOntologyProposedActionWire::SplitLabel => Self::SplitLabel,
            v1::DtoLabelOntologyProposedActionWire::MergeLabels => Self::MergeLabels,
            v1::DtoLabelOntologyProposedActionWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyReviewAtomVariantWire>
    for v1::DtoLabelOntologyReviewAtomVariantWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyReviewAtomVariantWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            content_hash: Some(dto.content_hash),
            polarity: dto.polarity,
            kind: dto.kind,
            text: dto.text,
            signal_count: Some(dto.signal_count),
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyReviewAtomVariantWire>
    for crate::label_surfaces::LabelOntologyReviewAtomVariantWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyReviewAtomVariantWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologyReviewAtomVariantWire {
            content_hash: required(wire.content_hash, "content_hash")?,
            polarity: wire.polarity,
            kind: wire.kind,
            text: wire.text,
            signal_count: required(wire.signal_count, "signal_count")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyReviewGroupByWire>
    for v1::DtoLabelOntologyReviewGroupByWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyReviewGroupByWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologyReviewGroupByWire::Label => Self::Label,
            crate::label_surfaces::LabelOntologyReviewGroupByWire::CandidateAtom => {
                Self::CandidateAtom
            }
            crate::label_surfaces::LabelOntologyReviewGroupByWire::ProposedLabel => {
                Self::ProposedLabel
            }
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyReviewGroupByWire>
    for crate::label_surfaces::LabelOntologyReviewGroupByWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyReviewGroupByWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologyReviewGroupByWire::Label => Self::Label,
            v1::DtoLabelOntologyReviewGroupByWire::CandidateAtom => Self::CandidateAtom,
            v1::DtoLabelOntologyReviewGroupByWire::ProposedLabel => Self::ProposedLabel,
            v1::DtoLabelOntologyReviewGroupByWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyReviewGroupWire>
    for v1::DtoLabelOntologyReviewGroupWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyReviewGroupWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            group_by: Some(i32::from(v1::DtoLabelOntologyReviewGroupByWire::try_from(
                dto.group_by,
            )?)),
            key: Some(dto.key),
            label_id: dto.label_id,
            label_name: dto.label_name,
            candidate_atom_polarity: dto.candidate_atom_polarity,
            candidate_atom_kind: dto.candidate_atom_kind,
            candidate_text: dto.candidate_text,
            candidate_content_hash: dto.candidate_content_hash,
            proposed_label_name: dto.proposed_label_name,
            proposed_label_name_normalized: dto.proposed_label_name_normalized,
            cluster_key: dto.cluster_key,
            cluster_reason: dto.cluster_reason,
            task_count: Some(dto.task_count),
            signal_count: Some(dto.signal_count),
            open_count: Some(dto.open_count),
            confirmed_count: Some(dto.confirmed_count),
            resolved_count: Some(dto.resolved_count),
            rejected_count: Some(dto.rejected_count),
            superseded_count: Some(dto.superseded_count),
            degraded_count: Some(dto.degraded_count),
            average_score: dto
                .average_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            median_score: dto
                .median_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            oldest_signal_at: Some(dto.oldest_signal_at),
            latest_signal_at: Some(dto.latest_signal_at),
            sample_task_refs: dto.sample_task_refs,
            signal_ids: dto.signal_ids,
            action_count: Some(dto.action_count),
            action_ids: dto.action_ids,
            proposal_ids: dto.proposal_ids,
            labels: (dto.labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            candidate_atom_variants: (dto.candidate_atom_variants)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyReviewGroupWire>
    for crate::label_surfaces::LabelOntologyReviewGroupWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyReviewGroupWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologyReviewGroupWire {
            group_by: v1::DtoLabelOntologyReviewGroupByWire::try_from(required(
                wire.group_by,
                "group_by",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            key: required(wire.key, "key")?,
            label_id: wire.label_id,
            label_name: wire.label_name,
            candidate_atom_polarity: wire.candidate_atom_polarity,
            candidate_atom_kind: wire.candidate_atom_kind,
            candidate_text: wire.candidate_text,
            candidate_content_hash: wire.candidate_content_hash,
            proposed_label_name: wire.proposed_label_name,
            proposed_label_name_normalized: wire.proposed_label_name_normalized,
            cluster_key: wire.cluster_key,
            cluster_reason: wire.cluster_reason,
            task_count: required(wire.task_count, "task_count")?,
            signal_count: required(wire.signal_count, "signal_count")?,
            open_count: required(wire.open_count, "open_count")?,
            confirmed_count: required(wire.confirmed_count, "confirmed_count")?,
            resolved_count: required(wire.resolved_count, "resolved_count")?,
            rejected_count: required(wire.rejected_count, "rejected_count")?,
            superseded_count: required(wire.superseded_count, "superseded_count")?,
            degraded_count: required(wire.degraded_count, "degraded_count")?,
            average_score: wire
                .average_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            median_score: wire
                .median_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            oldest_signal_at: required(wire.oldest_signal_at, "oldest_signal_at")?,
            latest_signal_at: required(wire.latest_signal_at, "latest_signal_at")?,
            sample_task_refs: wire.sample_task_refs,
            signal_ids: wire.signal_ids,
            action_count: required(wire.action_count, "action_count")?,
            action_ids: wire.action_ids,
            proposal_ids: wire.proposal_ids,
            labels: (wire.labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            candidate_atom_variants: (wire.candidate_atom_variants)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyReviewLabelRefWire>
    for v1::DtoLabelOntologyReviewLabelRefWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyReviewLabelRefWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            name: dto.name,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyReviewLabelRefWire>
    for crate::label_surfaces::LabelOntologyReviewLabelRefWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyReviewLabelRefWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologyReviewLabelRefWire {
            id: required(wire.id, "id")?,
            name: wire.name,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::LabelOntologyReviewMeta> for v1::DtoLabelOntologyReviewMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::LabelOntologyReviewMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            group_by: Some(dto.group_by),
            include_all: Some(dto.include_all),
            limit: Some(
                u64::try_from(dto.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyReviewMeta> for crate::wire::LabelOntologyReviewMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyReviewMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::LabelOntologyReviewMeta {
            group_by: required(wire.group_by, "group_by")?,
            include_all: required(wire.include_all, "include_all")?,
            limit: usize::try_from(required(wire.limit, "limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologySignalDetailWire>
    for v1::DtoLabelOntologySignalDetailWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologySignalDetailWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            signal: Some((dto.signal).try_into()?),
            observation: Some((dto.observation).try_into()?),
            actions: (dto.actions)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologySignalDetailWire>
    for crate::label_surfaces::LabelOntologySignalDetailWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologySignalDetailWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologySignalDetailWire {
            signal: (required(wire.signal, "signal")?).try_into()?,
            observation: (required(wire.observation, "observation")?).try_into()?,
            actions: (wire.actions)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologySignalKindWire>
    for v1::DtoLabelOntologySignalKindWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologySignalKindWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologySignalKindWire::FalseNegative => {
                Self::FalseNegative
            }
            crate::label_surfaces::LabelOntologySignalKindWire::FalsePositive => {
                Self::FalsePositive
            }
            crate::label_surfaces::LabelOntologySignalKindWire::VocabularyGap => {
                Self::VocabularyGap
            }
            crate::label_surfaces::LabelOntologySignalKindWire::NameIssue => Self::NameIssue,
            crate::label_surfaces::LabelOntologySignalKindWire::BoundaryIssue => {
                Self::BoundaryIssue
            }
            crate::label_surfaces::LabelOntologySignalKindWire::StructureIssue => {
                Self::StructureIssue
            }
        })
    }
}
impl TryFrom<v1::DtoLabelOntologySignalKindWire>
    for crate::label_surfaces::LabelOntologySignalKindWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologySignalKindWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologySignalKindWire::FalseNegative => Self::FalseNegative,
            v1::DtoLabelOntologySignalKindWire::FalsePositive => Self::FalsePositive,
            v1::DtoLabelOntologySignalKindWire::VocabularyGap => Self::VocabularyGap,
            v1::DtoLabelOntologySignalKindWire::NameIssue => Self::NameIssue,
            v1::DtoLabelOntologySignalKindWire::BoundaryIssue => Self::BoundaryIssue,
            v1::DtoLabelOntologySignalKindWire::StructureIssue => Self::StructureIssue,
            v1::DtoLabelOntologySignalKindWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologySignalRequest>
    for v1::DtoLabelOntologySignalRequest
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologySignalRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: Some(i32::from(v1::DtoLabelOntologySignalKindWire::try_from(
                dto.kind,
            )?)),
            target_label_ref: dto.target_label_ref,
            related_labels: match dto.related_labels {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
            proposed_action: Some(i32::from(v1::DtoLabelOntologyProposedActionWire::try_from(
                dto.proposed_action,
            )?)),
            candidate_atom: dto
                .candidate_atom
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            proposed_label_name: dto.proposed_label_name,
            proposal: match dto.proposal {
                crate::JsonBodyFieldWire::Missing => None,
                crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?),
            },
            agent_selected: Some(dto.agent_selected),
            suggest_state: dto
                .suggest_state
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoLabelOntologySuggestStateWire::try_from(
                        value,
                    )?))
                })
                .transpose()?,
            suggest_score: dto
                .suggest_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_rank: dto.suggest_rank,
            final_selected: Some(dto.final_selected),
            rationale: Some(dto.rationale),
            confidence: dto
                .confidence
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            signal_key: dto.signal_key,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologySignalRequest>
    for crate::label_surfaces::LabelOntologySignalRequest
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologySignalRequest) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelOntologySignalRequest {
            kind: v1::DtoLabelOntologySignalKindWire::try_from(required(wire.kind, "kind")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            target_label_ref: wire.target_label_ref,
            related_labels: match wire.related_labels {
                None => crate::JsonBodyFieldWire::Missing,
                Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
            },
            proposed_action: v1::DtoLabelOntologyProposedActionWire::try_from(required(
                wire.proposed_action,
                "proposed_action",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            candidate_atom: wire
                .candidate_atom
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            proposed_label_name: wire.proposed_label_name,
            proposal: match wire.proposal {
                None => crate::JsonBodyFieldWire::Missing,
                Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?),
            },
            agent_selected: required(wire.agent_selected, "agent_selected")?,
            suggest_state: wire
                .suggest_state
                .map(|value| -> Result<_, RpcCodecError> {
                    v1::DtoLabelOntologySuggestStateWire::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                        .try_into()
                })
                .transpose()?,
            suggest_score: wire
                .suggest_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_rank: wire.suggest_rank,
            final_selected: required(wire.final_selected, "final_selected")?,
            rationale: required(wire.rationale, "rationale")?,
            confidence: wire
                .confidence
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            signal_key: wire.signal_key,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::LabelOntologySignalReviewedPayload>
    for v1::DtoLabelOntologySignalReviewedPayload
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::event_payload::LabelOntologySignalReviewedPayload,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            signal_id: Some(dto.signal_id),
            action_id: Some(dto.action_id),
            status: Some(i32::from(v1::DtoSignalStatus::try_from(dto.status)?)),
            reason: Some(dto.reason),
        })
    }
}
impl TryFrom<v1::DtoLabelOntologySignalReviewedPayload>
    for crate::event_payload::LabelOntologySignalReviewedPayload
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologySignalReviewedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::LabelOntologySignalReviewedPayload {
            signal_id: required(wire.signal_id, "signal_id")?,
            action_id: required(wire.action_id, "action_id")?,
            status: v1::DtoSignalStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            reason: required(wire.reason, "reason")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::ontology::LabelOntologySignalWire> for v1::DtoLabelOntologySignalWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::ontology::LabelOntologySignalWire) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            observation_id: Some(dto.observation_id),
            board_id: Some(dto.board_id),
            kind: Some(dto.kind),
            status: Some(dto.status),
            target_label_id: dto.target_label_id,
            target_label_name_snapshot: dto.target_label_name_snapshot,
            related_labels: Some((dto.related_labels).try_into()?),
            proposed_action: Some(dto.proposed_action),
            candidate_atom_polarity: dto.candidate_atom_polarity,
            candidate_atom_kind: dto.candidate_atom_kind,
            candidate_text: dto.candidate_text,
            candidate_content_hash: dto.candidate_content_hash,
            proposed_label_name: dto.proposed_label_name,
            proposed_label_name_normalized: dto.proposed_label_name_normalized,
            proposal: Some((dto.proposal).try_into()?),
            agent_selected: Some(dto.agent_selected),
            suggest_state: dto.suggest_state,
            suggest_score: dto
                .suggest_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_rank: dto.suggest_rank,
            final_selected: Some(dto.final_selected),
            rationale: Some(dto.rationale),
            confidence: dto
                .confidence
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            signal_key: Some(dto.signal_key),
            superseded_by_signal_id: dto.superseded_by_signal_id,
            status_reason: dto.status_reason,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            reviewed_at: dto.reviewed_at,
            closed_at: dto.closed_at,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologySignalWire> for crate::ontology::LabelOntologySignalWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologySignalWire) -> Result<Self, Self::Error> {
        let result: Self = crate::ontology::LabelOntologySignalWire {
            id: required(wire.id, "id")?,
            observation_id: required(wire.observation_id, "observation_id")?,
            board_id: required(wire.board_id, "board_id")?,
            kind: required(wire.kind, "kind")?,
            status: required(wire.status, "status")?,
            target_label_id: wire.target_label_id,
            target_label_name_snapshot: wire.target_label_name_snapshot,
            related_labels: (required(wire.related_labels, "related_labels")?).try_into()?,
            proposed_action: required(wire.proposed_action, "proposed_action")?,
            candidate_atom_polarity: wire.candidate_atom_polarity,
            candidate_atom_kind: wire.candidate_atom_kind,
            candidate_text: wire.candidate_text,
            candidate_content_hash: wire.candidate_content_hash,
            proposed_label_name: wire.proposed_label_name,
            proposed_label_name_normalized: wire.proposed_label_name_normalized,
            proposal: (required(wire.proposal, "proposal")?).try_into()?,
            agent_selected: required(wire.agent_selected, "agent_selected")?,
            suggest_state: wire.suggest_state,
            suggest_score: wire
                .suggest_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_rank: wire.suggest_rank,
            final_selected: required(wire.final_selected, "final_selected")?,
            rationale: required(wire.rationale, "rationale")?,
            confidence: wire
                .confidence
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            signal_key: required(wire.signal_key, "signal_key")?,
            superseded_by_signal_id: wire.superseded_by_signal_id,
            status_reason: wire.status_reason,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            reviewed_at: wire.reviewed_at,
            closed_at: wire.closed_at,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologySuggestStateWire>
    for v1::DtoLabelOntologySuggestStateWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologySuggestStateWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologySuggestStateWire::Selected => Self::Selected,
            crate::label_surfaces::LabelOntologySuggestStateWire::Candidate => Self::Candidate,
            crate::label_surfaces::LabelOntologySuggestStateWire::Absent => Self::Absent,
            crate::label_surfaces::LabelOntologySuggestStateWire::Unavailable => Self::Unavailable,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologySuggestStateWire>
    for crate::label_surfaces::LabelOntologySuggestStateWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologySuggestStateWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologySuggestStateWire::Selected => Self::Selected,
            v1::DtoLabelOntologySuggestStateWire::Candidate => Self::Candidate,
            v1::DtoLabelOntologySuggestStateWire::Absent => Self::Absent,
            v1::DtoLabelOntologySuggestStateWire::Unavailable => Self::Unavailable,
            v1::DtoLabelOntologySuggestStateWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire>
    for v1::DtoLabelOntologyValidationEffectiveOutcomeWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire::NotRequired => {
                Self::NotRequired
            }
            crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire::Unsupported => {
                Self::Unsupported
            }
            crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire::Pending => {
                Self::Pending
            }
            crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire::Passed => {
                Self::Passed
            }
            crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire::Failed => {
                Self::Failed
            }
            crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire::Partial => {
                Self::Partial
            }
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyValidationEffectiveOutcomeWire>
    for crate::label_surfaces::LabelOntologyValidationEffectiveOutcomeWire
{
    type Error = RpcCodecError;
    fn try_from(
        wire: v1::DtoLabelOntologyValidationEffectiveOutcomeWire,
    ) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologyValidationEffectiveOutcomeWire::NotRequired => Self::NotRequired,
            v1::DtoLabelOntologyValidationEffectiveOutcomeWire::Unsupported => Self::Unsupported,
            v1::DtoLabelOntologyValidationEffectiveOutcomeWire::Pending => Self::Pending,
            v1::DtoLabelOntologyValidationEffectiveOutcomeWire::Passed => Self::Passed,
            v1::DtoLabelOntologyValidationEffectiveOutcomeWire::Failed => Self::Failed,
            v1::DtoLabelOntologyValidationEffectiveOutcomeWire::Partial => Self::Partial,
            v1::DtoLabelOntologyValidationEffectiveOutcomeWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyValidationRequirementWire>
    for v1::DtoLabelOntologyValidationRequirementWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyValidationRequirementWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologyValidationRequirementWire::None => Self::None,
            crate::label_surfaces::LabelOntologyValidationRequirementWire::Required => {
                Self::Required
            }
            crate::label_surfaces::LabelOntologyValidationRequirementWire::Unsupported => {
                Self::Unsupported
            }
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyValidationRequirementWire>
    for crate::label_surfaces::LabelOntologyValidationRequirementWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyValidationRequirementWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologyValidationRequirementWire::None => Self::None,
            v1::DtoLabelOntologyValidationRequirementWire::Required => Self::Required,
            v1::DtoLabelOntologyValidationRequirementWire::Unsupported => Self::Unsupported,
            v1::DtoLabelOntologyValidationRequirementWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelOntologyValidationStatusWire>
    for v1::DtoLabelOntologyValidationStatusWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelOntologyValidationStatusWire,
    ) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelOntologyValidationStatusWire::NotRequired => {
                Self::NotRequired
            }
            crate::label_surfaces::LabelOntologyValidationStatusWire::Pending => Self::Pending,
            crate::label_surfaces::LabelOntologyValidationStatusWire::Passed => Self::Passed,
            crate::label_surfaces::LabelOntologyValidationStatusWire::Failed => Self::Failed,
            crate::label_surfaces::LabelOntologyValidationStatusWire::Partial => Self::Partial,
        })
    }
}
impl TryFrom<v1::DtoLabelOntologyValidationStatusWire>
    for crate::label_surfaces::LabelOntologyValidationStatusWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelOntologyValidationStatusWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelOntologyValidationStatusWire::NotRequired => Self::NotRequired,
            v1::DtoLabelOntologyValidationStatusWire::Pending => Self::Pending,
            v1::DtoLabelOntologyValidationStatusWire::Passed => Self::Passed,
            v1::DtoLabelOntologyValidationStatusWire::Failed => Self::Failed,
            v1::DtoLabelOntologyValidationStatusWire::Partial => Self::Partial,
            v1::DtoLabelOntologyValidationStatusWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelProposalAttemptWire> for v1::DtoLabelProposalAttemptWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::LabelProposalAttemptWire) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: Some(dto.task_id),
            board_id: Some(dto.board_id),
            proposal: dto
                .proposal
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            degraded: Some(dto.degraded),
            diagnostics: dto.diagnostics,
            heuristic_coverage: Some(finite(dto.heuristic_coverage)?),
            heuristic_coverage_cosine: Some(finite(dto.heuristic_coverage_cosine)?),
            heuristic_residual_norm: Some(finite(dto.heuristic_residual_norm)?),
            top1_existing_label_id: dto.top1_existing_label_id,
            top1_existing_label_name: dto.top1_existing_label_name,
        })
    }
}
impl TryFrom<v1::DtoLabelProposalAttemptWire> for crate::label_surfaces::LabelProposalAttemptWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelProposalAttemptWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelProposalAttemptWire {
            task_id: required(wire.task_id, "task_id")?,
            board_id: required(wire.board_id, "board_id")?,
            proposal: wire
                .proposal
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            degraded: required(wire.degraded, "degraded")?,
            diagnostics: wire.diagnostics,
            heuristic_coverage: finite(required(wire.heuristic_coverage, "heuristic_coverage")?)?,
            heuristic_coverage_cosine: finite(required(
                wire.heuristic_coverage_cosine,
                "heuristic_coverage_cosine",
            )?)?,
            heuristic_residual_norm: finite(required(
                wire.heuristic_residual_norm,
                "heuristic_residual_norm",
            )?)?,
            top1_existing_label_id: wire.top1_existing_label_id,
            top1_existing_label_name: wire.top1_existing_label_name,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelProposalCandidateWire>
    for v1::DtoLabelProposalCandidateWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelProposalCandidateWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            name: Some(dto.name),
            description: dto.description,
            applies_when: dto.applies_when,
            excludes_when: dto.excludes_when,
            positive_examples: dto.positive_examples,
            negative_examples: dto.negative_examples,
        })
    }
}
impl TryFrom<v1::DtoLabelProposalCandidateWire>
    for crate::label_surfaces::LabelProposalCandidateWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelProposalCandidateWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelProposalCandidateWire {
            name: required(wire.name, "name")?,
            description: wire.description,
            applies_when: wire.applies_when,
            excludes_when: wire.excludes_when,
            positive_examples: wire.positive_examples,
            negative_examples: wire.negative_examples,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::LabelProposalPayload> for v1::DtoLabelProposalPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::LabelProposalPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            proposal_id: Some(dto.proposal_id),
            name: Some(dto.name),
            status: Some(i32::from(v1::DtoLabelProposalStatus::try_from(dto.status)?)),
        })
    }
}
impl TryFrom<v1::DtoLabelProposalPayload> for crate::event_payload::LabelProposalPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelProposalPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::LabelProposalPayload {
            proposal_id: required(wire.proposal_id, "proposal_id")?,
            name: required(wire.name, "name")?,
            status: v1::DtoLabelProposalStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::LabelProposalStatus> for v1::DtoLabelProposalStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::LabelProposalStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::LabelProposalStatus::Proposed => Self::Proposed,
            crate::event_payload::LabelProposalStatus::Accepted => Self::Accepted,
            crate::event_payload::LabelProposalStatus::Rejected => Self::Rejected,
        })
    }
}
impl TryFrom<v1::DtoLabelProposalStatus> for crate::event_payload::LabelProposalStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelProposalStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelProposalStatus::Proposed => Self::Proposed,
            v1::DtoLabelProposalStatus::Accepted => Self::Accepted,
            v1::DtoLabelProposalStatus::Rejected => Self::Rejected,
            v1::DtoLabelProposalStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelProposalStatusWire> for v1::DtoLabelProposalStatusWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::LabelProposalStatusWire) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::label_surfaces::LabelProposalStatusWire::Proposed => Self::Proposed,
            crate::label_surfaces::LabelProposalStatusWire::Accepted => Self::Accepted,
            crate::label_surfaces::LabelProposalStatusWire::Rejected => Self::Rejected,
        })
    }
}
impl TryFrom<v1::DtoLabelProposalStatusWire> for crate::label_surfaces::LabelProposalStatusWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelProposalStatusWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoLabelProposalStatusWire::Proposed => Self::Proposed,
            v1::DtoLabelProposalStatusWire::Accepted => Self::Accepted,
            v1::DtoLabelProposalStatusWire::Rejected => Self::Rejected,
            v1::DtoLabelProposalStatusWire::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::LabelSemanticProposalWire>
    for v1::DtoLabelSemanticProposalWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelSemanticProposalWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            task_id: Some(dto.task_id),
            status: Some(i32::from(v1::DtoLabelProposalStatusWire::try_from(
                dto.status,
            )?)),
            name: Some(dto.name),
            description: dto.description,
            applies_when: dto.applies_when,
            excludes_when: dto.excludes_when,
            positive_examples: dto.positive_examples,
            negative_examples: dto.negative_examples,
            heuristic_coverage: Some(finite(dto.heuristic_coverage)?),
            heuristic_coverage_cosine: Some(finite(dto.heuristic_coverage_cosine)?),
            heuristic_residual_norm: Some(finite(dto.heuristic_residual_norm)?),
            top1_existing_label_id: dto.top1_existing_label_id,
            top1_existing_label_name: dto.top1_existing_label_name,
            diagnostics: dto.diagnostics,
            created_by: Some(dto.created_by),
            decision_reason: dto.decision_reason,
            resolved_label_id: dto.resolved_label_id,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            decided_at: dto.decided_at,
        })
    }
}
impl TryFrom<v1::DtoLabelSemanticProposalWire>
    for crate::label_surfaces::LabelSemanticProposalWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelSemanticProposalWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelSemanticProposalWire {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            task_id: required(wire.task_id, "task_id")?,
            status: v1::DtoLabelProposalStatusWire::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            name: required(wire.name, "name")?,
            description: wire.description,
            applies_when: wire.applies_when,
            excludes_when: wire.excludes_when,
            positive_examples: wire.positive_examples,
            negative_examples: wire.negative_examples,
            heuristic_coverage: finite(required(wire.heuristic_coverage, "heuristic_coverage")?)?,
            heuristic_coverage_cosine: finite(required(
                wire.heuristic_coverage_cosine,
                "heuristic_coverage_cosine",
            )?)?,
            heuristic_residual_norm: finite(required(
                wire.heuristic_residual_norm,
                "heuristic_residual_norm",
            )?)?,
            top1_existing_label_id: wire.top1_existing_label_id,
            top1_existing_label_name: wire.top1_existing_label_name,
            diagnostics: wire.diagnostics,
            created_by: required(wire.created_by, "created_by")?,
            decision_reason: wire.decision_reason,
            resolved_label_id: wire.resolved_label_id,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            decided_at: wire.decided_at,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelSemanticsWire> for v1::DtoLabelSemanticsWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::LabelSemanticsWire) -> Result<Self, Self::Error> {
        Ok(Self {
            label_id: Some(dto.label_id),
            board_id: Some(dto.board_id),
            label_name: Some(dto.label_name),
            semantics_hash: Some(dto.semantics_hash),
            description: dto.description,
            applies_when: dto.applies_when,
            excludes_when: dto.excludes_when,
            positive_examples: dto.positive_examples,
            negative_examples: dto.negative_examples,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            atoms: (dto.atoms)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoLabelSemanticsWire> for crate::label_surfaces::LabelSemanticsWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelSemanticsWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelSemanticsWire {
            label_id: required(wire.label_id, "label_id")?,
            board_id: required(wire.board_id, "board_id")?,
            label_name: required(wire.label_name, "label_name")?,
            semantics_hash: required(wire.semantics_hash, "semantics_hash")?,
            description: wire.description,
            applies_when: wire.applies_when,
            excludes_when: wire.excludes_when,
            positive_examples: wire.positive_examples,
            negative_examples: wire.negative_examples,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            atoms: (wire.atoms)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelSuggestionCandidateWire>
    for v1::DtoLabelSuggestionCandidateWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelSuggestionCandidateWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            label_id: Some(dto.label_id),
            label_name: Some(dto.label_name),
            score: Some(finite(dto.score)?),
            weight: Some(finite(dto.weight)?),
            already_applied: Some(dto.already_applied),
            evidence_atoms: (dto.evidence_atoms)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            negative_evidence_atoms: (dto.negative_evidence_atoms)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoLabelSuggestionCandidateWire>
    for crate::label_surfaces::LabelSuggestionCandidateWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelSuggestionCandidateWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelSuggestionCandidateWire {
            label_id: required(wire.label_id, "label_id")?,
            label_name: required(wire.label_name, "label_name")?,
            score: finite(required(wire.score, "score")?)?,
            weight: finite(required(wire.weight, "weight")?)?,
            already_applied: required(wire.already_applied, "already_applied")?,
            evidence_atoms: (wire.evidence_atoms)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            negative_evidence_atoms: (wire.negative_evidence_atoms)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelSuggestionEvidenceAtomWire>
    for v1::DtoLabelSuggestionEvidenceAtomWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelSuggestionEvidenceAtomWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            atom_id: Some(dto.atom_id),
            label_id: Some(dto.label_id),
            label_name: Some(dto.label_name),
            polarity: Some(dto.polarity),
            kind: Some(dto.kind),
            text: Some(dto.text),
            score: Some(finite(dto.score)?),
        })
    }
}
impl TryFrom<v1::DtoLabelSuggestionEvidenceAtomWire>
    for crate::label_surfaces::LabelSuggestionEvidenceAtomWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelSuggestionEvidenceAtomWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelSuggestionEvidenceAtomWire {
            atom_id: required(wire.atom_id, "atom_id")?,
            label_id: required(wire.label_id, "label_id")?,
            label_name: required(wire.label_name, "label_name")?,
            polarity: required(wire.polarity, "polarity")?,
            kind: required(wire.kind, "kind")?,
            text: required(wire.text, "text")?,
            score: finite(required(wire.score, "score")?)?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::LabelSuggestionResultWire>
    for v1::DtoLabelSuggestionResultWire
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::label_surfaces::LabelSuggestionResultWire,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: Some(dto.task_id),
            board_id: Some(dto.board_id),
            selected_labels: (dto.selected_labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            candidates: (dto.candidates)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            coverage: Some(finite(dto.coverage)?),
            coverage_cosine: Some(finite(dto.coverage_cosine)?),
            residual_norm: Some(finite(dto.residual_norm)?),
            needs_new_label: Some(dto.needs_new_label),
            reason_codes: dto.reason_codes,
            degraded: Some(dto.degraded),
            diagnostics: dto.diagnostics,
        })
    }
}
impl TryFrom<v1::DtoLabelSuggestionResultWire>
    for crate::label_surfaces::LabelSuggestionResultWire
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLabelSuggestionResultWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::LabelSuggestionResultWire {
            task_id: required(wire.task_id, "task_id")?,
            board_id: required(wire.board_id, "board_id")?,
            selected_labels: (wire.selected_labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            candidates: (wire.candidates)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            coverage: finite(required(wire.coverage, "coverage")?)?,
            coverage_cosine: finite(required(wire.coverage_cosine, "coverage_cosine")?)?,
            residual_norm: finite(required(wire.residual_norm, "residual_norm")?)?,
            needs_new_label: required(wire.needs_new_label, "needs_new_label")?,
            reason_codes: wire.reason_codes,
            degraded: required(wire.degraded, "degraded")?,
            diagnostics: wire.diagnostics,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::LegacyImportReport> for v1::DtoLegacyImportReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::LegacyImportReport) -> Result<Self, Self::Error> {
        Ok(Self {
            journal_id: Some(dto.journal_id),
            phase: Some(dto.phase),
            source_path: Some(dto.source_path),
            source_fingerprint: Some(dto.source_fingerprint),
            schema_fingerprint: Some(dto.schema_fingerprint),
            resumed: Some(dto.resumed),
            attachment_count: Some(dto.attachment_count),
            table_counts: (dto.table_counts)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoLegacyImportReport> for crate::maintenance::LegacyImportReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLegacyImportReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::LegacyImportReport {
            journal_id: required(wire.journal_id, "journal_id")?,
            phase: required(wire.phase, "phase")?,
            source_path: required(wire.source_path, "source_path")?,
            source_fingerprint: required(wire.source_fingerprint, "source_fingerprint")?,
            schema_fingerprint: required(wire.schema_fingerprint, "schema_fingerprint")?,
            resumed: required(wire.resumed, "resumed")?,
            attachment_count: required(wire.attachment_count, "attachment_count")?,
            table_counts: (wire.table_counts)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::LegacyImportTableCount> for v1::DtoLegacyImportTableCount {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::LegacyImportTableCount) -> Result<Self, Self::Error> {
        Ok(Self {
            table: Some(dto.table),
            source_rows: Some(dto.source_rows),
            target_rows: Some(dto.target_rows),
        })
    }
}
impl TryFrom<v1::DtoLegacyImportTableCount> for crate::maintenance::LegacyImportTableCount {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLegacyImportTableCount) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::LegacyImportTableCount {
            table: required(wire.table, "table")?,
            source_rows: required(wire.source_rows, "source_rows")?,
            target_rows: required(wire.target_rows, "target_rows")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::LimitMeta> for v1::DtoLimitMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::LimitMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            limit: Some(
                u64::try_from(dto.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoLimitMeta> for crate::wire::LimitMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoLimitMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::LimitMeta {
            limit: usize::try_from(required(wire.limit, "limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::api_components::ListTasksByStatusData> for v1::DtoListTasksByStatusData {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ListTasksByStatusData) -> Result<Self, Self::Error> {
        Ok(Self {
            statuses: (dto.statuses)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoListTasksByStatusData> for crate::api_components::ListTasksByStatusData {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoListTasksByStatusData) -> Result<Self, Self::Error> {
        let result: Self = crate::api_components::ListTasksByStatusData {
            statuses: (wire.statuses)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::api_components::ListTasksStatusWindow> for v1::DtoListTasksStatusWindow {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ListTasksStatusWindow) -> Result<Self, Self::Error> {
        Ok(Self {
            status: Some(i32::from(v1::DtoApiTaskStatus::try_from(dto.status)?)),
            tasks: (dto.tasks)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            page: Some((dto.page).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoListTasksStatusWindow> for crate::api_components::ListTasksStatusWindow {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoListTasksStatusWindow) -> Result<Self, Self::Error> {
        let result: Self = crate::api_components::ListTasksStatusWindow {
            status: v1::DtoApiTaskStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            tasks: (wire.tasks)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            page: (required(wire.page, "page")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::MaintenanceOwnerStatus> for v1::DtoMaintenanceOwnerStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::MaintenanceOwnerStatus) -> Result<Self, Self::Error> {
        Ok(Self {
            owner: dto.owner,
            mode: dto.mode,
            lease_expires_at: dto.lease_expires_at,
            fence_epoch: Some(dto.fence_epoch),
            build_identity: dto.build_identity,
            last_heartbeat_at: dto.last_heartbeat_at,
            active: Some(dto.active),
        })
    }
}
impl TryFrom<v1::DtoMaintenanceOwnerStatus> for crate::maintenance::MaintenanceOwnerStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoMaintenanceOwnerStatus) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::MaintenanceOwnerStatus {
            owner: wire.owner,
            mode: wire.mode,
            lease_expires_at: wire.lease_expires_at,
            fence_epoch: required(wire.fence_epoch, "fence_epoch")?,
            build_identity: wire.build_identity,
            last_heartbeat_at: wire.last_heartbeat_at,
            active: required(wire.active, "active")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::MaintenanceRunReport> for v1::DtoMaintenanceRunReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::MaintenanceRunReport) -> Result<Self, Self::Error> {
        Ok(Self {
            database_instance_id: Some(dto.database_instance_id),
            protocol_version: Some(dto.protocol_version),
            owner: Some(dto.owner),
            mode: Some(dto.mode),
            action: Some(dto.action),
            processed: Some(dto.processed),
            phase: Some(dto.phase),
            degraded: Some(dto.degraded),
            errors: dto.errors,
            stores: (dto.stores)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoMaintenanceRunReport> for crate::maintenance::MaintenanceRunReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoMaintenanceRunReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::MaintenanceRunReport {
            database_instance_id: required(wire.database_instance_id, "database_instance_id")?,
            protocol_version: required(wire.protocol_version, "protocol_version")?,
            owner: required(wire.owner, "owner")?,
            mode: required(wire.mode, "mode")?,
            action: required(wire.action, "action")?,
            processed: required(wire.processed, "processed")?,
            phase: wire.phase.unwrap_or_default(),
            degraded: wire.degraded.unwrap_or_default(),
            errors: wire.errors,
            stores: (wire.stores)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::MaintenanceStatusReport> for v1::DtoMaintenanceStatusReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::MaintenanceStatusReport) -> Result<Self, Self::Error> {
        Ok(Self {
            database_instance_id: Some(dto.database_instance_id),
            protocol_version: Some(dto.protocol_version),
            owner: Some((dto.owner).try_into()?),
            stores: (dto.stores)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoMaintenanceStatusReport> for crate::maintenance::MaintenanceStatusReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoMaintenanceStatusReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::MaintenanceStatusReport {
            database_instance_id: required(wire.database_instance_id, "database_instance_id")?,
            protocol_version: required(wire.protocol_version, "protocol_version")?,
            owner: (required(wire.owner, "owner")?).try_into()?,
            stores: (wire.stores)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::NextAfterMeta> for v1::DtoNextAfterMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::NextAfterMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            next_after: Some(dto.next_after),
        })
    }
}
impl TryFrom<v1::DtoNextAfterMeta> for crate::wire::NextAfterMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoNextAfterMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::NextAfterMeta {
            next_after: required(wire.next_after, "next_after")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::OffsetPaginationMeta> for v1::DtoOffsetPaginationMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::OffsetPaginationMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            limit: Some(
                u64::try_from(dto.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            offset: Some(
                u64::try_from(dto.offset)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoOffsetPaginationMeta> for crate::wire::OffsetPaginationMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoOffsetPaginationMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::OffsetPaginationMeta {
            limit: usize::try_from(required(wire.limit, "limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            offset: usize::try_from(required(wire.offset, "offset")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::ProjectionStoreStatus> for v1::DtoProjectionStoreStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::ProjectionStoreStatus) -> Result<Self, Self::Error> {
        Ok(Self {
            store_name: Some(dto.store_name),
            active_generation: dto.active_generation,
            active_fingerprint: dto.active_fingerprint,
            previous_generation: dto.previous_generation,
            building_generation: dto.building_generation,
            lifecycle_status: Some(dto.lifecycle_status),
            fence_epoch: Some(dto.fence_epoch),
            last_event_id: Some(dto.last_event_id),
            dirty: Some(dto.dirty),
            pending: Some(dto.pending),
            running: Some(dto.running),
            failed: Some(dto.failed),
            last_error: dto.last_error,
            phase: Some(dto.phase),
            degraded: Some(dto.degraded),
            errors: dto.errors,
            updated_at: Some(dto.updated_at),
        })
    }
}
impl TryFrom<v1::DtoProjectionStoreStatus> for crate::maintenance::ProjectionStoreStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoProjectionStoreStatus) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::ProjectionStoreStatus {
            store_name: required(wire.store_name, "store_name")?,
            active_generation: wire.active_generation,
            active_fingerprint: wire.active_fingerprint,
            previous_generation: wire.previous_generation,
            building_generation: wire.building_generation,
            lifecycle_status: required(wire.lifecycle_status, "lifecycle_status")?,
            fence_epoch: required(wire.fence_epoch, "fence_epoch")?,
            last_event_id: required(wire.last_event_id, "last_event_id")?,
            dirty: required(wire.dirty, "dirty")?,
            pending: required(wire.pending, "pending")?,
            running: required(wire.running, "running")?,
            failed: required(wire.failed, "failed")?,
            last_error: wire.last_error,
            phase: wire.phase.unwrap_or_default(),
            degraded: wire.degraded.unwrap_or_default(),
            errors: wire.errors,
            updated_at: required(wire.updated_at, "updated_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::QueueStats> for v1::DtoQueueStats {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::QueueStats) -> Result<Self, Self::Error> {
        Ok(Self {
            board_id: Some(dto.board_id),
            generated_at: Some(dto.generated_at),
            status_counts: (dto.status_counts)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            stale_claims: (dto.stale_claims)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            blocked_reasons: (dto.blocked_reasons)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            unplanned_active_tasks: Some(dto.unplanned_active_tasks),
            active_parents_with_incomplete_required_steps: Some(
                dto.active_parents_with_incomplete_required_steps,
            ),
        })
    }
}
impl TryFrom<v1::DtoQueueStats> for crate::derived::QueueStats {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoQueueStats) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::QueueStats {
            board_id: required(wire.board_id, "board_id")?,
            generated_at: required(wire.generated_at, "generated_at")?,
            status_counts: (wire.status_counts)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            stale_claims: (wire.stale_claims)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            blocked_reasons: (wire.blocked_reasons)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            unplanned_active_tasks: required(
                wire.unplanned_active_tasks,
                "unplanned_active_tasks",
            )?,
            active_parents_with_incomplete_required_steps: required(
                wire.active_parents_with_incomplete_required_steps,
                "active_parents_with_incomplete_required_steps",
            )?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::lifecycle::ReclaimTargetStatus> for v1::DtoReclaimTargetStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::lifecycle::ReclaimTargetStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::lifecycle::ReclaimTargetStatus::Ready => Self::Ready,
            crate::lifecycle::ReclaimTargetStatus::Blocked => Self::Blocked,
        })
    }
}
impl TryFrom<v1::DtoReclaimTargetStatus> for crate::lifecycle::ReclaimTargetStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoReclaimTargetStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoReclaimTargetStatus::Ready => Self::Ready,
            v1::DtoReclaimTargetStatus::Blocked => Self::Blocked,
            v1::DtoReclaimTargetStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::event_payload::RetryPolicyPayload> for v1::DtoRetryPolicyPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::RetryPolicyPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            max_retries: dto.max_retries,
        })
    }
}
impl TryFrom<v1::DtoRetryPolicyPayload> for crate::event_payload::RetryPolicyPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoRetryPolicyPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::RetryPolicyPayload {
            max_retries: wire.max_retries,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::RunStatus> for v1::DtoRunStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::RunStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::RunStatus::Canceled => Self::Canceled,
        })
    }
}
impl TryFrom<v1::DtoRunStatus> for crate::event_payload::RunStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoRunStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoRunStatus::Canceled => Self::Canceled,
            v1::DtoRunStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::derived::SearchMeta> for v1::DtoSearchMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::SearchMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            backend: Some(dto.backend),
            stale: Some(dto.stale),
            database_instance_id: dto.database_instance_id,
            protocol_version: dto.protocol_version,
            generation: dto.generation,
            resolved_board_id: Some(dto.resolved_board_id),
            fallback_reason: dto.fallback_reason,
            index_version: dto.index_version,
            last_event_id: dto.last_event_id,
            index_lag_events: dto.index_lag_events,
        })
    }
}
impl TryFrom<v1::DtoSearchMeta> for crate::derived::SearchMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSearchMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::SearchMeta {
            backend: required(wire.backend, "backend")?,
            stale: required(wire.stale, "stale")?,
            database_instance_id: wire.database_instance_id,
            protocol_version: wire.protocol_version,
            generation: wire.generation,
            resolved_board_id: required(wire.resolved_board_id, "resolved_board_id")?,
            fallback_reason: wire.fallback_reason,
            index_version: wire.index_version,
            last_event_id: wire.last_event_id,
            index_lag_events: wire.index_lag_events,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::SearchPageMeta> for v1::DtoSearchPageMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::SearchPageMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            limit: Some(
                u64::try_from(dto.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            offset: Some(
                u64::try_from(dto.offset)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            total: dto
                .total
                .map(|value| -> Result<_, RpcCodecError> {
                    u64::try_from(value).map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))
                })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::DtoSearchPageMeta> for crate::derived::SearchPageMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSearchPageMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::SearchPageMeta {
            limit: usize::try_from(required(wire.limit, "limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            offset: usize::try_from(required(wire.offset, "offset")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            total: wire
                .total
                .map(|value| -> Result<_, RpcCodecError> {
                    usize::try_from(value).map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))
                })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::SearchStatus> for v1::DtoSearchStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::SearchStatus) -> Result<Self, Self::Error> {
        Ok(Self {
            backend: Some(dto.backend),
            derived_index: Some(dto.derived_index),
            stale: Some(dto.stale),
            database_instance_id: dto.database_instance_id,
            protocol_version: dto.protocol_version,
            generation: dto.generation,
            resolved_board_id: Some(dto.resolved_board_id),
            fallback_reason: dto.fallback_reason,
            index_version: dto.index_version,
            last_event_id: dto.last_event_id,
            index_lag_events: dto.index_lag_events,
            message: Some(dto.message),
        })
    }
}
impl TryFrom<v1::DtoSearchStatus> for crate::derived::SearchStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSearchStatus) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::SearchStatus {
            backend: required(wire.backend, "backend")?,
            derived_index: required(wire.derived_index, "derived_index")?,
            stale: required(wire.stale, "stale")?,
            database_instance_id: wire.database_instance_id,
            protocol_version: wire.protocol_version,
            generation: wire.generation,
            resolved_board_id: required(wire.resolved_board_id, "resolved_board_id")?,
            fallback_reason: wire.fallback_reason,
            index_version: wire.index_version,
            last_event_id: wire.last_event_id,
            index_lag_events: wire.index_lag_events,
            message: required(wire.message, "message")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::SearchTaskHit> for v1::DtoSearchTaskHit {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::SearchTaskHit) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: Some(dto.task_id),
            seq: Some(dto.seq),
            score: Some(finite(dto.score)?),
            snippet: dto.snippet,
            task: Some((dto.task).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoSearchTaskHit> for crate::derived::SearchTaskHit {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSearchTaskHit) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::SearchTaskHit {
            task_id: required(wire.task_id, "task_id")?,
            seq: required(wire.seq, "seq")?,
            score: finite(required(wire.score, "score")?)?,
            snippet: wire.snippet,
            task: (required(wire.task, "task")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::SearchTaskStatusWindow> for v1::DtoSearchTaskStatusWindow {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::SearchTaskStatusWindow) -> Result<Self, Self::Error> {
        Ok(Self {
            status: Some(i32::from(v1::DtoApiTaskStatus::try_from(dto.status)?)),
            tasks: (dto.tasks)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            search_meta: Some((dto.search_meta).try_into()?),
            page: Some((dto.page).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoSearchTaskStatusWindow> for crate::derived::SearchTaskStatusWindow {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSearchTaskStatusWindow) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::SearchTaskStatusWindow {
            status: v1::DtoApiTaskStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            tasks: (wire.tasks)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            search_meta: (required(wire.search_meta, "search_meta")?).try_into()?,
            page: (required(wire.page, "page")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::SearchTaskStatusWindows> for v1::DtoSearchTaskStatusWindows {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::SearchTaskStatusWindows) -> Result<Self, Self::Error> {
        Ok(Self {
            statuses: (dto.statuses)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoSearchTaskStatusWindows> for crate::derived::SearchTaskStatusWindows {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSearchTaskStatusWindows) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::SearchTaskStatusWindows {
            statuses: (wire.statuses)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::SearchTasksData> for v1::DtoSearchTasksData {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::SearchTasksData) -> Result<Self, Self::Error> {
        Ok(Self {
            hits: (dto.hits)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoSearchTasksData> for crate::derived::SearchTasksData {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSearchTasksData) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::SearchTasksData {
            hits: (wire.hits)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::signals::SignalCommentRequest> for v1::DtoSignalCommentRequest {
    type Error = RpcCodecError;
    fn try_from(dto: crate::signals::SignalCommentRequest) -> Result<Self, Self::Error> {
        Ok(Self { body: dto.body })
    }
}
impl TryFrom<v1::DtoSignalCommentRequest> for crate::signals::SignalCommentRequest {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalCommentRequest) -> Result<Self, Self::Error> {
        let result: Self = crate::signals::SignalCommentRequest { body: wire.body };
        Ok(result)
    }
}
impl TryFrom<crate::wire::SignalFilterMeta> for v1::DtoSignalFilterMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::SignalFilterMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            include_all: Some(dto.include_all),
            limit: Some(
                u64::try_from(dto.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoSignalFilterMeta> for crate::wire::SignalFilterMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalFilterMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::SignalFilterMeta {
            include_all: required(wire.include_all, "include_all")?,
            limit: usize::try_from(required(wire.limit, "limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::SignalObservationWire> for v1::DtoSignalObservationWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::SignalObservationWire) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            task_id: dto.task_id,
            task_ref_snapshot: dto.task_ref_snapshot,
            run_id: dto.run_id,
            comment_id: dto.comment_id,
            actor: Some(dto.actor),
            agent_type: dto.agent_type,
            source: dto.source,
            evidence: Some((dto.evidence).try_into()?),
            created_at: Some(dto.created_at),
        })
    }
}
impl TryFrom<v1::DtoSignalObservationWire> for crate::label_surfaces::SignalObservationWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalObservationWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::SignalObservationWire {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            task_id: wire.task_id,
            task_ref_snapshot: wire.task_ref_snapshot,
            run_id: wire.run_id,
            comment_id: wire.comment_id,
            actor: required(wire.actor, "actor")?,
            agent_type: wire.agent_type,
            source: wire.source,
            evidence: (required(wire.evidence, "evidence")?).try_into()?,
            created_at: required(wire.created_at, "created_at")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::signals::SignalRecordResult> for v1::DtoSignalRecordResult {
    type Error = RpcCodecError;
    fn try_from(dto: crate::signals::SignalRecordResult) -> Result<Self, Self::Error> {
        Ok(Self {
            signal: Some((dto.signal).try_into()?),
            backlink_comment: dto
                .backlink_comment
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::DtoSignalRecordResult> for crate::signals::SignalRecordResult {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalRecordResult) -> Result<Self, Self::Error> {
        let result: Self = crate::signals::SignalRecordResult {
            signal: (required(wire.signal, "signal")?).try_into()?,
            backlink_comment: wire
                .backlink_comment
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::SignalRecordedPayload> for v1::DtoSignalRecordedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::SignalRecordedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            signal_id: Some(dto.signal_id),
            observation_id: Some(dto.observation_id),
            kind: Some(dto.kind),
            status: Some(i32::from(v1::DtoSignalStatus::try_from(dto.status)?)),
        })
    }
}
impl TryFrom<v1::DtoSignalRecordedPayload> for crate::event_payload::SignalRecordedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalRecordedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::SignalRecordedPayload {
            signal_id: required(wire.signal_id, "signal_id")?,
            observation_id: required(wire.observation_id, "observation_id")?,
            kind: required(wire.kind, "kind")?,
            status: v1::DtoSignalStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::SignalReviewedPayload> for v1::DtoSignalReviewedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::SignalReviewedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            signal_id: Some(dto.signal_id),
            status: Some(i32::from(v1::DtoSignalStatus::try_from(dto.status)?)),
            reason: Some(dto.reason),
        })
    }
}
impl TryFrom<v1::DtoSignalReviewedPayload> for crate::event_payload::SignalReviewedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalReviewedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::SignalReviewedPayload {
            signal_id: required(wire.signal_id, "signal_id")?,
            status: v1::DtoSignalStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            reason: required(wire.reason, "reason")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::SignalStatus> for v1::DtoSignalStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::SignalStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::SignalStatus::Open => Self::Open,
            crate::event_payload::SignalStatus::Confirmed => Self::Confirmed,
            crate::event_payload::SignalStatus::Rejected => Self::Rejected,
            crate::event_payload::SignalStatus::Superseded => Self::Superseded,
            crate::event_payload::SignalStatus::Resolved => Self::Resolved,
        })
    }
}
impl TryFrom<v1::DtoSignalStatus> for crate::event_payload::SignalStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoSignalStatus::Open => Self::Open,
            v1::DtoSignalStatus::Confirmed => Self::Confirmed,
            v1::DtoSignalStatus::Rejected => Self::Rejected,
            v1::DtoSignalStatus::Superseded => Self::Superseded,
            v1::DtoSignalStatus::Resolved => Self::Resolved,
            v1::DtoSignalStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::label_surfaces::SignalWire> for v1::DtoSignalWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::SignalWire) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            board_id: Some(dto.board_id),
            observation_id: Some(dto.observation_id),
            kind: Some(dto.kind),
            title: Some(dto.title),
            summary: Some(dto.summary),
            severity: Some(dto.severity),
            status: Some(dto.status),
            dedupe_key: dto.dedupe_key,
            superseded_by_signal_id: dto.superseded_by_signal_id,
            reviewed_by: dto.reviewed_by,
            reviewed_at: dto.reviewed_at,
            review_reason: dto.review_reason,
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            observation: Some((dto.observation).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoSignalWire> for crate::label_surfaces::SignalWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoSignalWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::SignalWire {
            id: required(wire.id, "id")?,
            board_id: required(wire.board_id, "board_id")?,
            observation_id: required(wire.observation_id, "observation_id")?,
            kind: required(wire.kind, "kind")?,
            title: required(wire.title, "title")?,
            summary: required(wire.summary, "summary")?,
            severity: required(wire.severity, "severity")?,
            status: required(wire.status, "status")?,
            dedupe_key: wire.dedupe_key,
            superseded_by_signal_id: wire.superseded_by_signal_id,
            reviewed_by: wire.reviewed_by,
            reviewed_at: wire.reviewed_at,
            review_reason: wire.review_reason,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            observation: (required(wire.observation, "observation")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::StaleClaim> for v1::DtoStaleClaim {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::StaleClaim) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: Some(dto.task_id),
            seq: Some(dto.seq),
            title: Some(dto.title),
            claim_owner: dto.claim_owner,
            claim_expires_at: dto.claim_expires_at,
            last_heartbeat_at: dto.last_heartbeat_at,
            current_run_id: dto.current_run_id,
            retry_count: Some(dto.retry_count),
            max_retries: dto.max_retries,
        })
    }
}
impl TryFrom<v1::DtoStaleClaim> for crate::derived::StaleClaim {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoStaleClaim) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::StaleClaim {
            task_id: required(wire.task_id, "task_id")?,
            seq: required(wire.seq, "seq")?,
            title: required(wire.title, "title")?,
            claim_owner: wire.claim_owner,
            claim_expires_at: wire.claim_expires_at,
            last_heartbeat_at: wire.last_heartbeat_at,
            current_run_id: wire.current_run_id,
            retry_count: required(wire.retry_count, "retry_count")?,
            max_retries: wire.max_retries,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::StatusCount> for v1::DtoStatusCount {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::StatusCount) -> Result<Self, Self::Error> {
        Ok(Self {
            status: Some(i32::from(v1::DtoApiTaskStatus::try_from(dto.status)?)),
            count: Some(dto.count),
        })
    }
}
impl TryFrom<v1::DtoStatusCount> for crate::derived::StatusCount {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoStatusCount) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::StatusCount {
            status: v1::DtoApiTaskStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            count: required(wire.count, "count")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::StepStatus> for v1::DtoStepStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::StepStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::StepStatus::Todo => Self::Todo,
            crate::event_payload::StepStatus::Done => Self::Done,
            crate::event_payload::StepStatus::Skipped => Self::Skipped,
        })
    }
}
impl TryFrom<v1::DtoStepStatus> for crate::event_payload::StepStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoStepStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoStepStatus::Todo => Self::Todo,
            v1::DtoStepStatus::Done => Self::Done,
            v1::DtoStepStatus::Skipped => Self::Skipped,
            v1::DtoStepStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::event_stream::StreamEventData> for v1::DtoStreamEventData {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_stream::StreamEventData) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            event_id: Some(dto.event_id),
            board_id: Some(dto.board_id),
            task_id: dto.task_id,
            run_id: dto.run_id,
            kind: Some(dto.kind),
            actor: dto.actor,
            payload: Some((dto.payload).try_into()?),
            created_at: Some(dto.created_at),
        })
    }
}
impl TryFrom<v1::DtoStreamEventData> for crate::event_stream::StreamEventData {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoStreamEventData) -> Result<Self, Self::Error> {
        let result: Self = crate::event_stream::StreamEventData {
            id: required(wire.id, "id")?,
            event_id: required(wire.event_id, "event_id")?,
            board_id: required(wire.board_id, "board_id")?,
            task_id: wire.task_id,
            run_id: wire.run_id,
            kind: required(wire.kind, "kind")?,
            actor: wire.actor,
            payload: (required(wire.payload, "payload")?).try_into()?,
            created_at: required(wire.created_at, "created_at")?,
        };
        validate_stream_event(&result)?;
        Ok(result)
    }
}
impl TryFrom<crate::structured_metadata::JsonObject> for v1::DtoStructuredMetadataJsonObject {
    type Error = RpcCodecError;
    fn try_from(dto: crate::structured_metadata::JsonObject) -> Result<Self, Self::Error> {
        Ok(Self {
            value: (dto.0)
                .into_iter()
                .map(|(key, value)| -> Result<_, RpcCodecError> { Ok((key, encode_json(value)?)) })
                .collect::<Result<_, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoStructuredMetadataJsonObject> for crate::structured_metadata::JsonObject {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoStructuredMetadataJsonObject) -> Result<Self, Self::Error> {
        Ok(crate::structured_metadata::JsonObject(
            (wire.value)
                .into_iter()
                .map(|(key, value)| -> Result<_, RpcCodecError> { Ok((key, decode_json(value)?)) })
                .collect::<Result<_, _>>()?,
        ))
    }
}
impl TryFrom<crate::event_payload::TaskClaimedPayload> for v1::DtoTaskClaimedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskClaimedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            claim_owner: Some(dto.claim_owner),
            metadata: Some(encode_json(dto.metadata)?),
        })
    }
}
impl TryFrom<v1::DtoTaskClaimedPayload> for crate::event_payload::TaskClaimedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskClaimedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskClaimedPayload {
            claim_owner: required(wire.claim_owner, "claim_owner")?,
            metadata: decode_json(required(wire.metadata, "metadata")?)?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskCommentCreatedPayload> for v1::DtoTaskCommentCreatedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskCommentCreatedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            comment_id: Some(dto.comment_id),
            kind: Some(i32::from(v1::DtoEventPayloadCommentKind::try_from(
                dto.kind,
            )?)),
            author_type: Some(i32::from(v1::DtoEventPayloadCommentAuthorType::try_from(
                dto.author_type,
            )?)),
            agent_type: dto.agent_type,
        })
    }
}
impl TryFrom<v1::DtoTaskCommentCreatedPayload> for crate::event_payload::TaskCommentCreatedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskCommentCreatedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskCommentCreatedPayload {
            comment_id: required(wire.comment_id, "comment_id")?,
            kind: v1::DtoEventPayloadCommentKind::try_from(required(wire.kind, "kind")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            author_type: v1::DtoEventPayloadCommentAuthorType::try_from(required(
                wire.author_type,
                "author_type",
            )?)
            .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
            .try_into()?,
            agent_type: wire.agent_type,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_core::TaskDetailAggregate> for v1::DtoTaskDetailAggregate {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_core::TaskDetailAggregate) -> Result<Self, Self::Error> {
        Ok(Self {
            task: Some((dto.task).try_into()?),
            labels: (dto.labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            dependencies: Some((dto.dependencies).try_into()?),
            execution_plan: Some((dto.execution_plan).try_into()?),
            steps: (dto.steps)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            comments: (dto.comments)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            runs: (dto.runs)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            events: (dto.events)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            ontology: Some((dto.ontology).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoTaskDetailAggregate> for crate::task_core::TaskDetailAggregate {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskDetailAggregate) -> Result<Self, Self::Error> {
        let result: Self = crate::task_core::TaskDetailAggregate {
            task: (required(wire.task, "task")?).try_into()?,
            labels: (wire.labels)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            dependencies: (required(wire.dependencies, "dependencies")?).try_into()?,
            execution_plan: (required(wire.execution_plan, "execution_plan")?).try_into()?,
            steps: (wire.steps)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            comments: (wire.comments)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            runs: (wire.runs)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            events: (wire.events)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            ontology: (required(wire.ontology, "ontology")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_core::TaskDetailOntology> for v1::DtoTaskDetailOntology {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_core::TaskDetailOntology) -> Result<Self, Self::Error> {
        Ok(Self {
            summary: dto
                .summary
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            degraded: Some(dto.degraded),
            diagnostics: dto.diagnostics,
        })
    }
}
impl TryFrom<v1::DtoTaskDetailOntology> for crate::task_core::TaskDetailOntology {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskDetailOntology) -> Result<Self, Self::Error> {
        let result: Self = crate::task_core::TaskDetailOntology {
            summary: wire
                .summary
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
            degraded: required(wire.degraded, "degraded")?,
            diagnostics: wire.diagnostics,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskExportSanitizedPayload>
    for v1::DtoTaskExportSanitizedPayload
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::event_payload::TaskExportSanitizedPayload,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            from_status: Some(i32::from(v1::DtoTaskStatus::try_from(dto.from_status)?)),
            to_status: Some(i32::from(v1::DtoTaskStatus::try_from(dto.to_status)?)),
            run_status: Some(i32::from(v1::DtoRunStatus::try_from(dto.run_status)?)),
            original_run_id: dto.original_run_id,
            claim_owner: dto.claim_owner,
            claim_expires_at: dto.claim_expires_at,
            reason: Some(dto.reason),
        })
    }
}
impl TryFrom<v1::DtoTaskExportSanitizedPayload>
    for crate::event_payload::TaskExportSanitizedPayload
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskExportSanitizedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskExportSanitizedPayload {
            from_status: v1::DtoTaskStatus::try_from(required(wire.from_status, "from_status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            to_status: v1::DtoTaskStatus::try_from(required(wire.to_status, "to_status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            run_status: v1::DtoRunStatus::try_from(required(wire.run_status, "run_status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            original_run_id: wire.original_run_id,
            claim_owner: wire.claim_owner,
            claim_expires_at: wire.claim_expires_at,
            reason: required(wire.reason, "reason")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_graph::TaskGraphEdge> for v1::DtoTaskGraphEdge {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_graph::TaskGraphEdge) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            source_task_id: Some(dto.source_task_id),
            target_task_id: Some(dto.target_task_id),
            kind: Some(i32::from(v1::DtoApiTaskGraphEdgeKind::try_from(dto.kind)?)),
            required: Some(dto.required),
            blocking: Some(dto.blocking),
        })
    }
}
impl TryFrom<v1::DtoTaskGraphEdge> for crate::task_graph::TaskGraphEdge {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskGraphEdge) -> Result<Self, Self::Error> {
        let result: Self = crate::task_graph::TaskGraphEdge {
            id: required(wire.id, "id")?,
            source_task_id: required(wire.source_task_id, "source_task_id")?,
            target_task_id: required(wire.target_task_id, "target_task_id")?,
            kind: v1::DtoApiTaskGraphEdgeKind::try_from(required(wire.kind, "kind")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            required: required(wire.required, "required")?,
            blocking: required(wire.blocking, "blocking")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_graph::TaskGraphMeta> for v1::DtoTaskGraphMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_graph::TaskGraphMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            depth: Some(
                u64::try_from(dto.depth)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            context_depth: Some(
                u64::try_from(dto.context_depth)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            generated_at: Some(dto.generated_at),
            node_count: Some(
                u64::try_from(dto.node_count)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            edge_count: Some(
                u64::try_from(dto.edge_count)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            truncated: Some(dto.truncated),
            active_statuses: (dto.active_statuses)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(i32::from(v1::DtoApiTaskStatus::try_from(value)?))
                })
                .collect::<Result<Vec<_>, _>>()?,
            active_only: Some(dto.active_only),
            include_done_context: Some(dto.include_done_context),
            include_archived_context: Some(dto.include_archived_context),
            hide_isolated: Some(dto.hide_isolated),
            limit_nodes: Some(
                u64::try_from(dto.limit_nodes)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoTaskGraphMeta> for crate::task_graph::TaskGraphMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskGraphMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::task_graph::TaskGraphMeta {
            depth: usize::try_from(required(wire.depth, "depth")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            context_depth: usize::try_from(required(wire.context_depth, "context_depth")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            generated_at: required(wire.generated_at, "generated_at")?,
            node_count: usize::try_from(required(wire.node_count, "node_count")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            edge_count: usize::try_from(required(wire.edge_count, "edge_count")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            truncated: required(wire.truncated, "truncated")?,
            active_statuses: (wire.active_statuses)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> {
                    v1::DtoApiTaskStatus::try_from(value)
                        .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                        .try_into()
                })
                .collect::<Result<Vec<_>, _>>()?,
            active_only: required(wire.active_only, "active_only")?,
            include_done_context: required(wire.include_done_context, "include_done_context")?,
            include_archived_context: required(
                wire.include_archived_context,
                "include_archived_context",
            )?,
            hide_isolated: required(wire.hide_isolated, "hide_isolated")?,
            limit_nodes: usize::try_from(required(wire.limit_nodes, "limit_nodes")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_graph::TaskGraphNode> for v1::DtoTaskGraphNode {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_graph::TaskGraphNode) -> Result<Self, Self::Error> {
        Ok(Self {
            task: Some((dto.task).try_into()?),
            role: Some(i32::from(v1::DtoApiTaskGraphNodeRole::try_from(dto.role)?)),
            context_only: Some(dto.context_only),
        })
    }
}
impl TryFrom<v1::DtoTaskGraphNode> for crate::task_graph::TaskGraphNode {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskGraphNode) -> Result<Self, Self::Error> {
        let result: Self = crate::task_graph::TaskGraphNode {
            task: (required(wire.task, "task")?).try_into()?,
            role: v1::DtoApiTaskGraphNodeRole::try_from(required(wire.role, "role")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            context_only: required(wire.context_only, "context_only")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskLabelPayload> for v1::DtoTaskLabelPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskLabelPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            label_id: Some(dto.label_id),
            label: Some(dto.label),
        })
    }
}
impl TryFrom<v1::DtoTaskLabelPayload> for crate::event_payload::TaskLabelPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskLabelPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskLabelPayload {
            label_id: required(wire.label_id, "label_id")?,
            label: required(wire.label, "label")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_graph::TaskNeighborhood> for v1::DtoTaskNeighborhood {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_graph::TaskNeighborhood) -> Result<Self, Self::Error> {
        Ok(Self {
            center_task_id: Some(dto.center_task_id),
            nodes: (dto.nodes)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            edges: (dto.edges)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoTaskNeighborhood> for crate::task_graph::TaskNeighborhood {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskNeighborhood) -> Result<Self, Self::Error> {
        let result: Self = crate::task_graph::TaskNeighborhood {
            center_task_id: required(wire.center_task_id, "center_task_id")?,
            nodes: (wire.nodes)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            edges: (wire.edges)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::TaskOntologyDetailsMeta<Option<crate::task_core::TaskOntologySummary>>>
    for v1::DtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::TaskOntologyDetailsMeta<Option<crate::task_core::TaskOntologySummary>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            details: Some((dto.details).try_into()?),
        })
    }
}
impl TryFrom<v1::DtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary>
    for crate::wire::TaskOntologyDetailsMeta<Option<crate::task_core::TaskOntologySummary>>
{
    type Error = RpcCodecError;
    fn try_from(
        wire: v1::DtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary,
    ) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::TaskOntologyDetailsMeta {
            details: (required(wire.details, "details")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::TaskOntologyDetails<Option<crate::task_core::TaskOntologySummary>>>
    for v1::DtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::TaskOntologyDetails<Option<crate::task_core::TaskOntologySummary>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            ontology_summary: dto
                .ontology_summary
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::DtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary>
    for crate::wire::TaskOntologyDetails<Option<crate::task_core::TaskOntologySummary>>
{
    type Error = RpcCodecError;
    fn try_from(
        wire: v1::DtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary,
    ) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::TaskOntologyDetails {
            ontology_summary: wire
                .ontology_summary
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_core::TaskOntologySignalSummary> for v1::DtoTaskOntologySignalSummary {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_core::TaskOntologySignalSummary) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            kind: Some(dto.kind),
            status: Some(dto.status),
            proposed_action: Some(dto.proposed_action),
            target_label_id: dto.target_label_id,
            target_label_name: dto.target_label_name,
            candidate_atom_polarity: dto.candidate_atom_polarity,
            candidate_atom_kind: dto.candidate_atom_kind,
            candidate_text: dto.candidate_text,
            candidate_content_hash: dto.candidate_content_hash,
            proposed_label_name: dto.proposed_label_name,
            proposed_label_name_normalized: dto.proposed_label_name_normalized,
            suggest_score: dto
                .suggest_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_rank: dto.suggest_rank,
            degraded: Some(dto.degraded),
            stale: Some(dto.stale),
            legacy_incomparable: Some(dto.legacy_incomparable),
            suggest_input_drift: Some(dto.suggest_input_drift),
            created_at: Some(dto.created_at),
            updated_at: Some(dto.updated_at),
            latest_action_at: dto.latest_action_at,
            action_count: Some(dto.action_count),
        })
    }
}
impl TryFrom<v1::DtoTaskOntologySignalSummary> for crate::task_core::TaskOntologySignalSummary {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskOntologySignalSummary) -> Result<Self, Self::Error> {
        let result: Self = crate::task_core::TaskOntologySignalSummary {
            id: required(wire.id, "id")?,
            kind: required(wire.kind, "kind")?,
            status: required(wire.status, "status")?,
            proposed_action: required(wire.proposed_action, "proposed_action")?,
            target_label_id: wire.target_label_id,
            target_label_name: wire.target_label_name,
            candidate_atom_polarity: wire.candidate_atom_polarity,
            candidate_atom_kind: wire.candidate_atom_kind,
            candidate_text: wire.candidate_text,
            candidate_content_hash: wire.candidate_content_hash,
            proposed_label_name: wire.proposed_label_name,
            proposed_label_name_normalized: wire.proposed_label_name_normalized,
            suggest_score: wire
                .suggest_score
                .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                .transpose()?,
            suggest_rank: wire.suggest_rank,
            degraded: required(wire.degraded, "degraded")?,
            stale: required(wire.stale, "stale")?,
            legacy_incomparable: required(wire.legacy_incomparable, "legacy_incomparable")?,
            suggest_input_drift: required(wire.suggest_input_drift, "suggest_input_drift")?,
            created_at: required(wire.created_at, "created_at")?,
            updated_at: required(wire.updated_at, "updated_at")?,
            latest_action_at: wire.latest_action_at,
            action_count: required(wire.action_count, "action_count")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_core::TaskOntologySummary> for v1::DtoTaskOntologySummary {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_core::TaskOntologySummary) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: Some(dto.task_id),
            observation_count: Some(dto.observation_count),
            signal_count: Some(dto.signal_count),
            open_count: Some(dto.open_count),
            confirmed_count: Some(dto.confirmed_count),
            resolved_count: Some(dto.resolved_count),
            rejected_count: Some(dto.rejected_count),
            superseded_count: Some(dto.superseded_count),
            degraded_count: Some(dto.degraded_count),
            stale_count: Some(dto.stale_count),
            suggest_input_drift_count: Some(dto.suggest_input_drift_count),
            legacy_incomparable_count: Some(dto.legacy_incomparable_count),
            incomparable_count: Some(dto.incomparable_count),
            action_count: Some(dto.action_count),
            oldest_open_confirmed_signal_at: dto.oldest_open_confirmed_signal_at,
            oldest_open_confirmed_signal_age_ms: dto.oldest_open_confirmed_signal_age_ms,
            latest_signal_at: dto.latest_signal_at,
            latest_action_at: dto.latest_action_at,
            current_suggest_input_hash: Some(dto.current_suggest_input_hash),
            sample_signals: (dto.sample_signals)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::DtoTaskOntologySummary> for crate::task_core::TaskOntologySummary {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskOntologySummary) -> Result<Self, Self::Error> {
        let result: Self = crate::task_core::TaskOntologySummary {
            task_id: required(wire.task_id, "task_id")?,
            observation_count: required(wire.observation_count, "observation_count")?,
            signal_count: required(wire.signal_count, "signal_count")?,
            open_count: required(wire.open_count, "open_count")?,
            confirmed_count: required(wire.confirmed_count, "confirmed_count")?,
            resolved_count: required(wire.resolved_count, "resolved_count")?,
            rejected_count: required(wire.rejected_count, "rejected_count")?,
            superseded_count: required(wire.superseded_count, "superseded_count")?,
            degraded_count: required(wire.degraded_count, "degraded_count")?,
            stale_count: required(wire.stale_count, "stale_count")?,
            suggest_input_drift_count: required(
                wire.suggest_input_drift_count,
                "suggest_input_drift_count",
            )?,
            legacy_incomparable_count: required(
                wire.legacy_incomparable_count,
                "legacy_incomparable_count",
            )?,
            incomparable_count: required(wire.incomparable_count, "incomparable_count")?,
            action_count: required(wire.action_count, "action_count")?,
            oldest_open_confirmed_signal_at: wire.oldest_open_confirmed_signal_at,
            oldest_open_confirmed_signal_age_ms: wire.oldest_open_confirmed_signal_age_ms,
            latest_signal_at: wire.latest_signal_at,
            latest_action_at: wire.latest_action_at,
            current_suggest_input_hash: required(
                wire.current_suggest_input_hash,
                "current_suggest_input_hash",
            )?,
            sample_signals: (wire.sample_signals)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_read::TaskReadLabel> for v1::DtoTaskReadLabel {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_read::TaskReadLabel) -> Result<Self, Self::Error> {
        Ok(Self {
            value: Some(dto.into_string()),
        })
    }
}
impl TryFrom<v1::DtoTaskReadLabel> for crate::task_read::TaskReadLabel {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskReadLabel) -> Result<Self, Self::Error> {
        crate::task_read::TaskReadLabel::new(required(wire.value, "value")?)
            .ok_or_else(|| RpcCodecError::invalid("TaskReadLabel"))
    }
}
impl TryFrom<crate::task_read::TaskReadPlanFilter> for v1::DtoTaskReadPlanFilter {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_read::TaskReadPlanFilter) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::task_read::TaskReadPlanFilter::PlanNeeded => Self::PlanNeeded,
            crate::task_read::TaskReadPlanFilter::HasSteps => Self::HasSteps,
            crate::task_read::TaskReadPlanFilter::IncompleteRequiredSteps => {
                Self::IncompleteRequiredSteps
            }
        })
    }
}
impl TryFrom<v1::DtoTaskReadPlanFilter> for crate::task_read::TaskReadPlanFilter {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskReadPlanFilter) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoTaskReadPlanFilter::PlanNeeded => Self::PlanNeeded,
            v1::DtoTaskReadPlanFilter::HasSteps => Self::HasSteps,
            v1::DtoTaskReadPlanFilter::IncompleteRequiredSteps => Self::IncompleteRequiredSteps,
            v1::DtoTaskReadPlanFilter::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::task_read::TaskReadSort> for v1::DtoTaskReadSort {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_read::TaskReadSort) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::task_read::TaskReadSort::Seq => Self::Seq,
            crate::task_read::TaskReadSort::SeqDesc => Self::SeqDesc,
            crate::task_read::TaskReadSort::Title => Self::Title,
            crate::task_read::TaskReadSort::TitleDesc => Self::TitleDesc,
            crate::task_read::TaskReadSort::Status => Self::Status,
            crate::task_read::TaskReadSort::StatusDesc => Self::StatusDesc,
            crate::task_read::TaskReadSort::Position => Self::Position,
            crate::task_read::TaskReadSort::PositionDesc => Self::PositionDesc,
            crate::task_read::TaskReadSort::Priority => Self::Priority,
            crate::task_read::TaskReadSort::PriorityDesc => Self::PriorityDesc,
            crate::task_read::TaskReadSort::Assignee => Self::Assignee,
            crate::task_read::TaskReadSort::AssigneeDesc => Self::AssigneeDesc,
            crate::task_read::TaskReadSort::ScheduledAt => Self::ScheduledAt,
            crate::task_read::TaskReadSort::ScheduledAtDesc => Self::ScheduledAtDesc,
            crate::task_read::TaskReadSort::DueAt => Self::DueAt,
            crate::task_read::TaskReadSort::DueAtDesc => Self::DueAtDesc,
            crate::task_read::TaskReadSort::CreatedAt => Self::CreatedAt,
            crate::task_read::TaskReadSort::CreatedAtDesc => Self::CreatedAtDesc,
            crate::task_read::TaskReadSort::UpdatedAt => Self::UpdatedAt,
            crate::task_read::TaskReadSort::UpdatedAtDesc => Self::UpdatedAtDesc,
        })
    }
}
impl TryFrom<v1::DtoTaskReadSort> for crate::task_read::TaskReadSort {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskReadSort) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoTaskReadSort::Seq => Self::Seq,
            v1::DtoTaskReadSort::SeqDesc => Self::SeqDesc,
            v1::DtoTaskReadSort::Title => Self::Title,
            v1::DtoTaskReadSort::TitleDesc => Self::TitleDesc,
            v1::DtoTaskReadSort::Status => Self::Status,
            v1::DtoTaskReadSort::StatusDesc => Self::StatusDesc,
            v1::DtoTaskReadSort::Position => Self::Position,
            v1::DtoTaskReadSort::PositionDesc => Self::PositionDesc,
            v1::DtoTaskReadSort::Priority => Self::Priority,
            v1::DtoTaskReadSort::PriorityDesc => Self::PriorityDesc,
            v1::DtoTaskReadSort::Assignee => Self::Assignee,
            v1::DtoTaskReadSort::AssigneeDesc => Self::AssigneeDesc,
            v1::DtoTaskReadSort::ScheduledAt => Self::ScheduledAt,
            v1::DtoTaskReadSort::ScheduledAtDesc => Self::ScheduledAtDesc,
            v1::DtoTaskReadSort::DueAt => Self::DueAt,
            v1::DtoTaskReadSort::DueAtDesc => Self::DueAtDesc,
            v1::DtoTaskReadSort::CreatedAt => Self::CreatedAt,
            v1::DtoTaskReadSort::CreatedAtDesc => Self::CreatedAtDesc,
            v1::DtoTaskReadSort::UpdatedAt => Self::UpdatedAt,
            v1::DtoTaskReadSort::UpdatedAtDesc => Self::UpdatedAtDesc,
            v1::DtoTaskReadSort::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::event_payload::TaskReasonPayload> for v1::DtoTaskReasonPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskReasonPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            reason: Some(dto.reason),
        })
    }
}
impl TryFrom<v1::DtoTaskReasonPayload> for crate::event_payload::TaskReasonPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskReasonPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskReasonPayload {
            reason: required(wire.reason, "reason")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskReclaimedPayload> for v1::DtoTaskReclaimedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskReclaimedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            retry_count: Some(dto.retry_count),
            max_retries: dto.max_retries,
            to_status: Some(i32::from(v1::DtoTaskStatus::try_from(dto.to_status)?)),
            reason: Some(dto.reason),
        })
    }
}
impl TryFrom<v1::DtoTaskReclaimedPayload> for crate::event_payload::TaskReclaimedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskReclaimedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskReclaimedPayload {
            retry_count: required(wire.retry_count, "retry_count")?,
            max_retries: wire.max_retries,
            to_status: v1::DtoTaskStatus::try_from(required(wire.to_status, "to_status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            reason: required(wire.reason, "reason")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskReopenedPayload> for v1::DtoTaskReopenedPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskReopenedPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            from: Some(i32::from(v1::DtoTaskStatus::try_from(dto.from)?)),
            to: Some(i32::from(v1::DtoTaskStatus::try_from(dto.to)?)),
            reason: Some(dto.reason),
            original_completed_at: dto.original_completed_at,
        })
    }
}
impl TryFrom<v1::DtoTaskReopenedPayload> for crate::event_payload::TaskReopenedPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskReopenedPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskReopenedPayload {
            from: v1::DtoTaskStatus::try_from(required(wire.from, "from")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            to: v1::DtoTaskStatus::try_from(required(wire.to, "to")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
            reason: required(wire.reason, "reason")?,
            original_completed_at: wire.original_completed_at,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskResultPayload> for v1::DtoTaskResultPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskResultPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            result: dto
                .result
                .map(|value| -> Result<_, RpcCodecError> { encode_json(value) })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::DtoTaskResultPayload> for crate::event_payload::TaskResultPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskResultPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskResultPayload {
            result: wire
                .result
                .map(|value| -> Result<_, RpcCodecError> { decode_json(value) })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskRetryPayload> for v1::DtoTaskRetryPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskRetryPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            retry_count: Some(dto.retry_count),
            max_retries: dto.max_retries,
        })
    }
}
impl TryFrom<v1::DtoTaskRetryPayload> for crate::event_payload::TaskRetryPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskRetryPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskRetryPayload {
            retry_count: required(wire.retry_count, "retry_count")?,
            max_retries: wire.max_retries,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskStatus> for v1::DtoTaskStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskStatus) -> Result<Self, Self::Error> {
        Ok(match dto {
            crate::event_payload::TaskStatus::Triage => Self::Triage,
            crate::event_payload::TaskStatus::Todo => Self::Todo,
            crate::event_payload::TaskStatus::Scheduled => Self::Scheduled,
            crate::event_payload::TaskStatus::Ready => Self::Ready,
            crate::event_payload::TaskStatus::Running => Self::Running,
            crate::event_payload::TaskStatus::Blocked => Self::Blocked,
            crate::event_payload::TaskStatus::Review => Self::Review,
            crate::event_payload::TaskStatus::Done => Self::Done,
            crate::event_payload::TaskStatus::Archived => Self::Archived,
        })
    }
}
impl TryFrom<v1::DtoTaskStatus> for crate::event_payload::TaskStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskStatus) -> Result<Self, Self::Error> {
        Ok(match wire {
            v1::DtoTaskStatus::Triage => Self::Triage,
            v1::DtoTaskStatus::Todo => Self::Todo,
            v1::DtoTaskStatus::Scheduled => Self::Scheduled,
            v1::DtoTaskStatus::Ready => Self::Ready,
            v1::DtoTaskStatus::Running => Self::Running,
            v1::DtoTaskStatus::Blocked => Self::Blocked,
            v1::DtoTaskStatus::Review => Self::Review,
            v1::DtoTaskStatus::Done => Self::Done,
            v1::DtoTaskStatus::Archived => Self::Archived,
            v1::DtoTaskStatus::Unspecified => {
                return Err(RpcCodecError::invalid("enum 未指定"));
            }
        })
    }
}
impl TryFrom<crate::event_payload::TaskStatusPayload> for v1::DtoTaskStatusPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskStatusPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            status: Some(i32::from(v1::DtoTaskStatus::try_from(dto.status)?)),
        })
    }
}
impl TryFrom<v1::DtoTaskStatusPayload> for crate::event_payload::TaskStatusPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskStatusPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskStatusPayload {
            status: v1::DtoTaskStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskStepPayload> for v1::DtoTaskStepPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskStepPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            step_id: Some(dto.step_id),
            linked_task_id: dto.linked_task_id,
            position: Some(dto.position),
            required: Some(dto.required),
            status: Some(i32::from(v1::DtoStepStatus::try_from(dto.status)?)),
        })
    }
}
impl TryFrom<v1::DtoTaskStepPayload> for crate::event_payload::TaskStepPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskStepPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskStepPayload {
            step_id: required(wire.step_id, "step_id")?,
            linked_task_id: wire.linked_task_id,
            position: required(wire.position, "position")?,
            required: required(wire.required, "required")?,
            status: v1::DtoStepStatus::try_from(required(wire.status, "status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::event_payload::TaskToStatusPayload> for v1::DtoTaskToStatusPayload {
    type Error = RpcCodecError;
    fn try_from(dto: crate::event_payload::TaskToStatusPayload) -> Result<Self, Self::Error> {
        Ok(Self {
            to_status: Some(i32::from(v1::DtoTaskStatus::try_from(dto.to_status)?)),
        })
    }
}
impl TryFrom<v1::DtoTaskToStatusPayload> for crate::event_payload::TaskToStatusPayload {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTaskToStatusPayload) -> Result<Self, Self::Error> {
        let result: Self = crate::event_payload::TaskToStatusPayload {
            to_status: v1::DtoTaskStatus::try_from(required(wire.to_status, "to_status")?)
                .map_err(|_| RpcCodecError::invalid("未知 enum 数值"))?
                .try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::TotalPaginationMeta> for v1::DtoTotalPaginationMeta {
    type Error = RpcCodecError;
    fn try_from(dto: crate::wire::TotalPaginationMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            limit: Some(
                u64::try_from(dto.limit)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            offset: Some(
                u64::try_from(dto.offset)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
            total: Some(
                u64::try_from(dto.total)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoTotalPaginationMeta> for crate::wire::TotalPaginationMeta {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoTotalPaginationMeta) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::TotalPaginationMeta {
            limit: usize::try_from(required(wire.limit, "limit")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            offset: usize::try_from(required(wire.offset, "offset")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
            total: usize::try_from(required(wire.total, "total")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::maintenance::VacuumReport> for v1::DtoVacuumReport {
    type Error = RpcCodecError;
    fn try_from(dto: crate::maintenance::VacuumReport) -> Result<Self, Self::Error> {
        Ok(Self {
            ok: Some(dto.ok),
            before_bytes: Some(dto.before_bytes),
            after_bytes: Some(dto.after_bytes),
            source_fingerprint: Some(dto.source_fingerprint),
        })
    }
}
impl TryFrom<v1::DtoVacuumReport> for crate::maintenance::VacuumReport {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoVacuumReport) -> Result<Self, Self::Error> {
        let result: Self = crate::maintenance::VacuumReport {
            ok: required(wire.ok, "ok")?,
            before_bytes: required(wire.before_bytes, "before_bytes")?,
            after_bytes: required(wire.after_bytes, "after_bytes")?,
            source_fingerprint: required(wire.source_fingerprint, "source_fingerprint")?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::protocols::VectorChunkResult> for v1::DtoVectorChunkResult {
    type Error = RpcCodecError;
    fn try_from(dto: crate::protocols::VectorChunkResult) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(dto.id),
            entity_uri: dto.entity_uri,
            source_kind: Some(dto.source_kind),
            content: Some(dto.content),
            content_hash: Some(dto.content_hash),
            embedding_model: Some(dto.embedding_model),
            distance: Some(finite(dto.distance)?),
            score: Some(finite(dto.score)?),
        })
    }
}
impl TryFrom<v1::DtoVectorChunkResult> for crate::protocols::VectorChunkResult {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoVectorChunkResult) -> Result<Self, Self::Error> {
        let result: Self = crate::protocols::VectorChunkResult {
            id: required(wire.id, "id")?,
            entity_uri: wire.entity_uri,
            source_kind: required(wire.source_kind, "source_kind")?,
            content: required(wire.content, "content")?,
            content_hash: required(wire.content_hash, "content_hash")?,
            embedding_model: required(wire.embedding_model, "embedding_model")?,
            distance: finite(required(wire.distance, "distance")?)?,
            score: finite(required(wire.score, "score")?)?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::protocols::VectorConfigureRequest> for v1::DtoVectorConfigureRequest {
    type Error = RpcCodecError;
    fn try_from(dto: crate::protocols::VectorConfigureRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            provider: Some(dto.provider),
            endpoint: Some(dto.endpoint),
            model: Some(dto.model),
            dimensions: Some(
                u64::try_from(dto.dimensions)
                    .map_err(|_| RpcCodecError::invalid("usize 超出 uint64"))?,
            ),
        })
    }
}
impl TryFrom<v1::DtoVectorConfigureRequest> for crate::protocols::VectorConfigureRequest {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoVectorConfigureRequest) -> Result<Self, Self::Error> {
        let result: Self = crate::protocols::VectorConfigureRequest {
            provider: required(wire.provider, "provider")?,
            endpoint: required(wire.endpoint, "endpoint")?,
            model: required(wire.model, "model")?,
            dimensions: usize::try_from(required(wire.dimensions, "dimensions")?)
                .map_err(|_| RpcCodecError::invalid("uint64 超出 usize"))?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::protocols::VectorLabelAtomResult> for v1::DtoVectorLabelAtomResult {
    type Error = RpcCodecError;
    fn try_from(dto: crate::protocols::VectorLabelAtomResult) -> Result<Self, Self::Error> {
        Ok(Self {
            atom_id: Some(dto.atom_id),
            label_id: Some(dto.label_id),
            label_name: Some(dto.label_name),
            board_id: Some(dto.board_id),
            polarity: Some(dto.polarity),
            kind: Some(dto.kind),
            text: Some(dto.text),
            ordinal: Some(dto.ordinal),
            content_hash: Some(dto.content_hash),
            embedding_model: Some(dto.embedding_model),
            distance: Some(finite(dto.distance)?),
            vector: dto
                .vector
                .map(|value| -> Result<_, RpcCodecError> {
                    Ok(v1::ListOfF32 {
                        items: (value)
                            .into_iter()
                            .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::DtoVectorLabelAtomResult> for crate::protocols::VectorLabelAtomResult {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoVectorLabelAtomResult) -> Result<Self, Self::Error> {
        let result: Self = crate::protocols::VectorLabelAtomResult {
            atom_id: required(wire.atom_id, "atom_id")?,
            label_id: required(wire.label_id, "label_id")?,
            label_name: required(wire.label_name, "label_name")?,
            board_id: required(wire.board_id, "board_id")?,
            polarity: required(wire.polarity, "polarity")?,
            kind: required(wire.kind, "kind")?,
            text: required(wire.text, "text")?,
            ordinal: required(wire.ordinal, "ordinal")?,
            content_hash: required(wire.content_hash, "content_hash")?,
            embedding_model: required(wire.embedding_model, "embedding_model")?,
            distance: finite(required(wire.distance, "distance")?)?,
            vector: wire
                .vector
                .map(|value| -> Result<_, RpcCodecError> {
                    ((value).items)
                        .into_iter()
                        .map(|value| -> Result<_, RpcCodecError> { finite(value) })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::derived::VectorStatus> for v1::DtoVectorStatus {
    type Error = RpcCodecError;
    fn try_from(dto: crate::derived::VectorStatus) -> Result<Self, Self::Error> {
        Ok(Self {
            backend: Some(dto.backend),
            enabled: Some(dto.enabled),
            message: Some(dto.message),
            diagnostics: dto.diagnostics,
            dirty: dto.dirty,
            board_dirty: dto.board_dirty,
            generation: dto.generation,
        })
    }
}
impl TryFrom<v1::DtoVectorStatus> for crate::derived::VectorStatus {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoVectorStatus) -> Result<Self, Self::Error> {
        let result: Self = crate::derived::VectorStatus {
            backend: required(wire.backend, "backend")?,
            enabled: required(wire.enabled, "enabled")?,
            message: required(wire.message, "message")?,
            diagnostics: wire.diagnostics,
            dirty: wire.dirty,
            board_dirty: wire.board_dirty,
            generation: wire.generation,
        };
        Ok(result)
    }
}
impl TryFrom<crate::label_surfaces::VectorStoreStatusWire> for v1::DtoVectorStoreStatusWire {
    type Error = RpcCodecError;
    fn try_from(dto: crate::label_surfaces::VectorStoreStatusWire) -> Result<Self, Self::Error> {
        Ok(Self {
            backend: Some(dto.backend),
            enabled: Some(dto.enabled),
            message: Some(dto.message),
            diagnostics: dto.diagnostics,
            dirty: dto.dirty,
            board_dirty: dto.board_dirty,
            generation: dto.generation,
        })
    }
}
impl TryFrom<v1::DtoVectorStoreStatusWire> for crate::label_surfaces::VectorStoreStatusWire {
    type Error = RpcCodecError;
    fn try_from(wire: v1::DtoVectorStoreStatusWire) -> Result<Self, Self::Error> {
        let result: Self = crate::label_surfaces::VectorStoreStatusWire {
            backend: required(wire.backend, "backend")?,
            enabled: required(wire.enabled, "enabled")?,
            message: required(wire.message, "message")?,
            diagnostics: wire.diagnostics,
            dirty: wire.dirty,
            board_dirty: wire.board_dirty,
            generation: wire.generation,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelAtomExplainWire>>
    for v1::ExplainLabelAtomResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelAtomExplainWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ExplainLabelAtomResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelAtomExplainWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ExplainLabelAtomResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::boards::GetBoardResponse> for v1::GetBoardResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::boards::GetBoardResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetBoardResponse> for crate::boards::GetBoardResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetBoardResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::boards::GetBoardResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::cli::CliEntity>> for v1::GetEntityResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::cli::CliEntity>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetEntityResponse> for crate::wire::DataEnvelope<crate::cli::CliEntity> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetEntityResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::wire::HealthReport>> for v1::GetHealthResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::wire::HealthReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetHealthResponse> for crate::wire::DataEnvelope<crate::wire::HealthReport> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetHealthResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::cli_labels::CliLabelOntologyQuality>>
    for v1::GetLabelOntologyQualityResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::cli_labels::CliLabelOntologyQuality>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetLabelOntologyQualityResponse>
    for crate::wire::DataEnvelope<crate::cli_labels::CliLabelOntologyQuality>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetLabelOntologyQualityResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologySignalDetailWire>>
    for v1::GetLabelOntologySignalResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologySignalDetailWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetLabelOntologySignalResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologySignalDetailWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetLabelOntologySignalResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>>
    for v1::GetLabelProposalResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetLabelProposalResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetLabelProposalResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticsWire>>
    for v1::GetLabelSemanticsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticsWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetLabelSemanticsResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticsWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetLabelSemanticsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::runs::GetRunLogResponse> for v1::GetRunLogResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::runs::GetRunLogResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetRunLogResponse> for crate::runs::GetRunLogResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetRunLogResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::runs::GetRunLogResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::runs::GetRunResponse> for v1::GetRunResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::runs::GetRunResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetRunResponse> for crate::runs::GetRunResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetRunResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::runs::GetRunResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::SignalWire>>
    for v1::GetSignalResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::SignalWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetSignalResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::SignalWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetSignalResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::QueueStats>> for v1::GetStatsResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::QueueStats>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetStatsResponse> for crate::wire::DataEnvelope<crate::derived::QueueStats> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetStatsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::task_core::GetTaskDetailsResponse> for v1::GetTaskDetailsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::task_core::GetTaskDetailsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GetTaskDetailsResponse> for crate::task_core::GetTaskDetailsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetTaskDetailsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::task_core::GetTaskDetailsResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<
        crate::wire::OptionalMetadataEnvelope<
            crate::api_components::ApiTask,
            crate::wire::TaskOntologyDetailsMeta<Option<crate::task_core::TaskOntologySummary>>,
        >,
    > for v1::GetTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::OptionalMetadataEnvelope<
            crate::api_components::ApiTask,
            crate::wire::TaskOntologyDetailsMeta<Option<crate::task_core::TaskOntologySummary>>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
            meta: dto
                .meta
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        })
    }
}
impl TryFrom<v1::GetTaskResponse>
    for crate::wire::OptionalMetadataEnvelope<
        crate::api_components::ApiTask,
        crate::wire::TaskOntologyDetailsMeta<Option<crate::task_core::TaskOntologySummary>>,
    >
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GetTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::OptionalMetadataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
            meta: wire
                .meta
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .transpose()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<crate::wire::MetadataEnvelope<Vec<crate::derived::ApiRelation>, crate::wire::LimitMeta>>
    for v1::GraphNeighborsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::MetadataEnvelope<
            Vec<crate::derived::ApiRelation>,
            crate::wire::LimitMeta,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::GraphNeighborsResponse>
    for crate::wire::MetadataEnvelope<Vec<crate::derived::ApiRelation>, crate::wire::LimitMeta>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GraphNeighborsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::MetadataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::cli_helpers::CliGraphQueryRow>>>
    for v1::GraphQueryResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::cli_helpers::CliGraphQueryRow>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::GraphQueryResponse>
    for crate::wire::DataEnvelope<Vec<crate::cli_helpers::CliGraphQueryRow>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GraphQueryResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::GraphMaintenance>>
    for v1::GraphRebuildResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::GraphMaintenance>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GraphRebuildResponse>
    for crate::wire::DataEnvelope<crate::derived::GraphMaintenance>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GraphRebuildResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::GraphStatus>> for v1::GraphStatusResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::GraphStatus>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GraphStatusResponse> for crate::wire::DataEnvelope<crate::derived::GraphStatus> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::GraphStatusResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::GraphMaintenance>>
    for v1::GraphSyncResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::GraphMaintenance>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::GraphSyncResponse>
    for crate::wire::DataEnvelope<crate::derived::GraphMaintenance>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::GraphSyncResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::HeartbeatTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::HeartbeatTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::HeartbeatTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::VectorStoreStatusWire>>
    for v1::LabelAtomIndexStatusResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::VectorStoreStatusWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::LabelAtomIndexStatusResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::VectorStoreStatusWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::LabelAtomIndexStatusResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::attachments::ApiAttachment>>>
    for v1::ListAttachmentsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::attachments::ApiAttachment>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListAttachmentsResponse>
    for crate::wire::DataEnvelope<Vec<crate::attachments::ApiAttachment>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListAttachmentsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::boards::ListBoardColumnsResponse> for v1::ListBoardColumnsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::boards::ListBoardColumnsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListBoardColumnsResponse> for crate::boards::ListBoardColumnsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListBoardColumnsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::boards::ListBoardColumnsResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticProposalWire>>>
    for v1::ListBoardLabelProposalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticProposalWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListBoardLabelProposalsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticProposalWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListBoardLabelProposalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::api_components::ApiLabel>>>
    for v1::ListBoardLabelsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::api_components::ApiLabel>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListBoardLabelsResponse>
    for crate::wire::DataEnvelope<Vec<crate::api_components::ApiLabel>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListBoardLabelsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::boards::ListBoardsResponse> for v1::ListBoardsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::boards::ListBoardsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListBoardsResponse> for crate::boards::ListBoardsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListBoardsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::boards::ListBoardsResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::comments::ListCommentsResponse> for v1::ListCommentsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::comments::ListCommentsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListCommentsResponse> for crate::comments::ListCommentsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListCommentsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::comments::ListCommentsResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::dependencies::ListDependenciesResponse> for v1::ListDependenciesResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::dependencies::ListDependenciesResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ListDependenciesResponse> for crate::dependencies::ListDependenciesResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListDependenciesResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::dependencies::ListDependenciesResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::cli::CliEntity>>> for v1::ListEntitiesResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::cli::CliEntity>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListEntitiesResponse> for crate::wire::DataEnvelope<Vec<crate::cli::CliEntity>> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListEntitiesResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::events::ListEventsResponse> for v1::ListEventsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::events::ListEventsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::ListEventsResponse> for crate::events::ListEventsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListEventsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::events::ListEventsResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelAtomWire>>>
    for v1::ListLabelAtomsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelAtomWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListLabelAtomsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelAtomWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListLabelAtomsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::ontology::LabelOntologySignalsResponse>
    for v1::ListLabelOntologySignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(dto: crate::ontology::LabelOntologySignalsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::ListLabelOntologySignalsResponse>
    for crate::ontology::LabelOntologySignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListLabelOntologySignalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::ontology::LabelOntologySignalsResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticsWire>>>
    for v1::ListLabelSemanticsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticsWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListLabelSemanticsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticsWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListLabelSemanticsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::runs::ListRunsResponse> for v1::ListRunsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::runs::ListRunsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListRunsResponse> for crate::runs::ListRunsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListRunsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::runs::ListRunsResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<
        crate::wire::MetadataEnvelope<
            Vec<crate::label_surfaces::SignalWire>,
            crate::wire::SignalFilterMeta,
        >,
    > for v1::ListSignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::MetadataEnvelope<
            Vec<crate::label_surfaces::SignalWire>,
            crate::wire::SignalFilterMeta,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::ListSignalsResponse>
    for crate::wire::MetadataEnvelope<
        Vec<crate::label_surfaces::SignalWire>,
        crate::wire::SignalFilterMeta,
    >
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListSignalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::MetadataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::ListStepsResponse> for v1::ListStepsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::ListStepsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ListStepsResponse> for crate::steps::ListStepsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListStepsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::ListStepsResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticProposalWire>>>
    for v1::ListTaskLabelProposalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticProposalWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListTaskLabelProposalsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::LabelSemanticProposalWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListTaskLabelProposalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::labels::ListTaskLabelsResponse> for v1::ListTaskLabelsResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::labels::ListTaskLabelsResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ListTaskLabelsResponse> for crate::labels::ListTaskLabelsResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListTaskLabelsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::labels::ListTaskLabelsResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::api_components::ListTasksByStatusResponse> for v1::ListTasksByStatusResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::api_components::ListTasksByStatusResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::ListTasksByStatusResponse> for crate::api_components::ListTasksByStatusResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListTasksByStatusResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::api_components::ListTasksByStatusResponse {
            data: (required(wire.data, "data")?).try_into()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::api_components::ListTasksResponse> for v1::ListTasksResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::api_components::ListTasksResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::ListTasksResponse> for crate::api_components::ListTasksResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ListTasksResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::api_components::ListTasksResponse {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::BackupReport>>
    for v1::MaintenanceBackupResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::BackupReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceBackupResponse>
    for crate::wire::DataEnvelope<crate::maintenance::BackupReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceBackupResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>>
    for v1::MaintenanceCleanupResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceCleanupResponse>
    for crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceCleanupResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::ExportReport>>
    for v1::MaintenanceExportResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::ExportReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceExportResponse>
    for crate::wire::DataEnvelope<crate::maintenance::ExportReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceExportResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::ImportReport>>
    for v1::MaintenanceImportResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::ImportReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceImportResponse>
    for crate::wire::DataEnvelope<crate::maintenance::ImportReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceImportResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::LegacyImportReport>>
    for v1::MaintenanceImportV30Response
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::LegacyImportReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceImportV30Response>
    for crate::wire::DataEnvelope<crate::maintenance::LegacyImportReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceImportV30Response) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>>
    for v1::MaintenanceRebuildResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceRebuildResponse>
    for crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceRebuildResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>>
    for v1::MaintenanceRunResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceRunResponse>
    for crate::wire::DataEnvelope<crate::maintenance::MaintenanceRunReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceRunResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::MaintenanceStatusReport>>
    for v1::MaintenanceStatusResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::MaintenanceStatusReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceStatusResponse>
    for crate::wire::DataEnvelope<crate::maintenance::MaintenanceStatusReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceStatusResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::maintenance::VacuumReport>>
    for v1::MaintenanceVacuumResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::maintenance::VacuumReport>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MaintenanceVacuumResponse>
    for crate::wire::DataEnvelope<crate::maintenance::VacuumReport>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MaintenanceVacuumResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::MarkExecutionPlanNotRequiredResponse>
    for v1::MarkExecutionPlanNotRequiredResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::steps::MarkExecutionPlanNotRequiredResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::MarkExecutionPlanNotRequiredResponse>
    for crate::steps::MarkExecutionPlanNotRequiredResponse
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::MarkExecutionPlanNotRequiredResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::MarkExecutionPlanNotRequiredResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::PromoteTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::PromoteTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::PromoteTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelProposalAttemptWire>>
    for v1::ProposeTaskLabelResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelProposalAttemptWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ProposeTaskLabelResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelProposalAttemptWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ProposeTaskLabelResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::rpc::dto::LabelAtomIndexQueryData>>
    for v1::QueryLabelAtomIndexResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::rpc::dto::LabelAtomIndexQueryData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::QueryLabelAtomIndexResponse>
    for crate::wire::DataEnvelope<crate::rpc::dto::LabelAtomIndexQueryData>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::QueryLabelAtomIndexResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::VectorStoreStatusWire>>
    for v1::RebuildLabelAtomIndexResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::VectorStoreStatusWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RebuildLabelAtomIndexResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::VectorStoreStatusWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::RebuildLabelAtomIndexResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::SearchStatus>>
    for v1::RebuildSearchIndexResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::SearchStatus>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RebuildSearchIndexResponse>
    for crate::wire::DataEnvelope<crate::derived::SearchStatus>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::RebuildSearchIndexResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::ReclaimTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ReclaimTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ReclaimTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyObservationWire>>
    for v1::RecordLabelOntologyObservationResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyObservationWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RecordLabelOntologyObservationResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyObservationWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::RecordLabelOntologyObservationResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::signals::SignalRecordResult>>
    for v1::RecordSignalResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::signals::SignalRecordResult>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RecordSignalResponse>
    for crate::wire::DataEnvelope<crate::signals::SignalRecordResult>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::RecordSignalResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>>
    for v1::RejectLabelProposalResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RejectLabelProposalResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticProposalWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::RejectLabelProposalResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>>
    for v1::RejectSignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::RejectSignalsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::RejectSignalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::ReleaseTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ReleaseTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ReleaseTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::dependencies::RemoveDependencyResponse> for v1::RemoveDependencyResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::dependencies::RemoveDependencyResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RemoveDependencyResponse> for crate::dependencies::RemoveDependencyResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::RemoveDependencyResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::dependencies::RemoveDependencyResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::RemoveStepResponse> for v1::RemoveStepResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::RemoveStepResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RemoveStepResponse> for crate::steps::RemoveStepResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::RemoveStepResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::RemoveStepResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::labels::RemoveTaskLabelResponse> for v1::RemoveTaskLabelResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::labels::RemoveTaskLabelResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RemoveTaskLabelResponse> for crate::labels::RemoveTaskLabelResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::RemoveTaskLabelResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::labels::RemoveTaskLabelResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::ReopenStepResponse> for v1::ReopenStepResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::ReopenStepResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ReopenStepResponse> for crate::steps::ReopenStepResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ReopenStepResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::ReopenStepResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>> for v1::ReopenTaskResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ReopenTaskResponse> for crate::wire::DataEnvelope<crate::api_components::ApiTask> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::ReopenTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>>
    for v1::ResolveSignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::ResolveSignalsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ResolveSignalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>>
    for v1::RevertLabelOntologyMutationResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::RevertLabelOntologyMutationResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::RevertLabelOntologyMutationResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<
        crate::wire::MetadataEnvelope<
            Vec<crate::label_surfaces::LabelOntologyReviewGroupWire>,
            crate::wire::LabelOntologyReviewMeta,
        >,
    > for v1::ReviewLabelOntologyResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::MetadataEnvelope<
            Vec<crate::label_surfaces::LabelOntologyReviewGroupWire>,
            crate::wire::LabelOntologyReviewMeta,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::ReviewLabelOntologyResponse>
    for crate::wire::MetadataEnvelope<
        Vec<crate::label_surfaces::LabelOntologyReviewGroupWire>,
        crate::wire::LabelOntologyReviewMeta,
    >
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ReviewLabelOntologyResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::MetadataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<
        crate::wire::MetadataEnvelope<
            Vec<crate::label_surfaces::SignalWire>,
            crate::wire::SignalFilterMeta,
        >,
    > for v1::ReviewSignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::MetadataEnvelope<
            Vec<crate::label_surfaces::SignalWire>,
            crate::wire::SignalFilterMeta,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::ReviewSignalsResponse>
    for crate::wire::MetadataEnvelope<
        Vec<crate::label_surfaces::SignalWire>,
        crate::wire::SignalFilterMeta,
    >
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ReviewSignalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::MetadataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::SearchStatus>> for v1::SearchStatusResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::SearchStatus>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::SearchStatusResponse> for crate::wire::DataEnvelope<crate::derived::SearchStatus> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::SearchStatusResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<
        crate::wire::MetadataEnvelope<
            crate::derived::SearchTaskStatusWindows,
            crate::wire::OffsetPaginationMeta,
        >,
    > for v1::SearchTasksByStatusResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::MetadataEnvelope<
            crate::derived::SearchTaskStatusWindows,
            crate::wire::OffsetPaginationMeta,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::SearchTasksByStatusResponse>
    for crate::wire::MetadataEnvelope<
        crate::derived::SearchTaskStatusWindows,
        crate::wire::OffsetPaginationMeta,
    >
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::SearchTasksByStatusResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::MetadataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl
    TryFrom<
        crate::wire::MetadataEnvelope<
            crate::derived::SearchTasksData,
            crate::wire::OffsetPaginationMeta,
        >,
    > for v1::SearchTasksResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::MetadataEnvelope<
            crate::derived::SearchTasksData,
            crate::wire::OffsetPaginationMeta,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
            meta: Some((dto.meta).try_into()?),
        })
    }
}
impl TryFrom<v1::SearchTasksResponse>
    for crate::wire::MetadataEnvelope<
        crate::derived::SearchTasksData,
        crate::wire::OffsetPaginationMeta,
    >
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::SearchTasksResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::MetadataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
            meta: (required(wire.meta, "meta")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::SkipStepResponse> for v1::SkipStepResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::SkipStepResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::SkipStepResponse> for crate::steps::SkipStepResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::SkipStepResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::SkipStepResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::SpecifyTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::SpecifyTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::SpecifyTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::SubmitReviewTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::SubmitReviewTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::SubmitReviewTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelSuggestionResultWire>>
    for v1::SuggestTaskLabelsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelSuggestionResultWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::SuggestTaskLabelsResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelSuggestionResultWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::SuggestTaskLabelsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>>
    for v1::SupersedeSignalsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::SupersedeSignalsResponse>
    for crate::wire::DataEnvelope<Vec<crate::label_surfaces::SignalWire>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::SupersedeSignalsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::SearchStatus>>
    for v1::SyncSearchIndexResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::SearchStatus>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::SyncSearchIndexResponse>
    for crate::wire::DataEnvelope<crate::derived::SearchStatus>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::SyncSearchIndexResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::task_graph::TaskNeighborhood>>
    for v1::TaskNeighborhoodResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::task_graph::TaskNeighborhood>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::TaskNeighborhoodResponse>
    for crate::wire::DataEnvelope<crate::task_graph::TaskNeighborhood>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::TaskNeighborhoodResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>>
    for v1::UnblockTaskResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::UnblockTaskResponse>
    for crate::wire::DataEnvelope<crate::api_components::ApiTask>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::UnblockTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::steps::UpdateStepResponse> for v1::UpdateStepResponse {
    type Error = RpcCodecError;
    fn try_from(dto: crate::steps::UpdateStepResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::UpdateStepResponse> for crate::steps::UpdateStepResponse {
    type Error = RpcCodecError;
    fn try_from(wire: v1::UpdateStepResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::steps::UpdateStepResponse {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::api_components::ApiTask>> for v1::UpdateTaskResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::api_components::ApiTask>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::UpdateTaskResponse> for crate::wire::DataEnvelope<crate::api_components::ApiTask> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::UpdateTaskResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::cli::CliEntity>> for v1::UpsertEntityResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::cli::CliEntity>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::UpsertEntityResponse> for crate::wire::DataEnvelope<crate::cli::CliEntity> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::UpsertEntityResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticsWire>>
    for v1::UpsertLabelSemanticsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticsWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::UpsertLabelSemanticsResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelSemanticsWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::UpsertLabelSemanticsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>>
    for v1::ValidateLabelOntologyActionResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::ValidateLabelOntologyActionResponse>
    for crate::wire::DataEnvelope<crate::label_surfaces::LabelOntologyActionWire>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::ValidateLabelOntologyActionResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::protocols::VectorConfigureRequest>>
    for v1::VectorConfigureResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::protocols::VectorConfigureRequest>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::VectorConfigureResponse>
    for crate::wire::DataEnvelope<crate::protocols::VectorConfigureRequest>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::VectorConfigureResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::protocols::VectorChunkResult>>>
    for v1::VectorQueryChunksResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::protocols::VectorChunkResult>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::VectorQueryChunksResponse>
    for crate::wire::DataEnvelope<Vec<crate::protocols::VectorChunkResult>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::VectorQueryChunksResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<Vec<crate::protocols::VectorLabelAtomResult>>>
    for v1::VectorQueryLabelAtomsResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<Vec<crate::protocols::VectorLabelAtomResult>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: (dto.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl TryFrom<v1::VectorQueryLabelAtomsResponse>
    for crate::wire::DataEnvelope<Vec<crate::protocols::VectorLabelAtomResult>>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::VectorQueryLabelAtomsResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (wire.data)
                .into_iter()
                .map(|value| -> Result<_, RpcCodecError> { (value).try_into() })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::VectorStatus>>
    for v1::VectorRebuildResponse
{
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::VectorStatus>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::VectorRebuildResponse>
    for crate::wire::DataEnvelope<crate::derived::VectorStatus>
{
    type Error = RpcCodecError;
    fn try_from(wire: v1::VectorRebuildResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::VectorStatus>> for v1::VectorStatusResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::VectorStatus>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::VectorStatusResponse> for crate::wire::DataEnvelope<crate::derived::VectorStatus> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::VectorStatusResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
impl TryFrom<crate::wire::DataEnvelope<crate::derived::VectorStatus>> for v1::VectorSyncResponse {
    type Error = RpcCodecError;
    fn try_from(
        dto: crate::wire::DataEnvelope<crate::derived::VectorStatus>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: Some((dto.data).try_into()?),
        })
    }
}
impl TryFrom<v1::VectorSyncResponse> for crate::wire::DataEnvelope<crate::derived::VectorStatus> {
    type Error = RpcCodecError;
    fn try_from(wire: v1::VectorSyncResponse) -> Result<Self, Self::Error> {
        let result: Self = crate::wire::DataEnvelope {
            data: (required(wire.data, "data")?).try_into()?,
        };
        Ok(result)
    }
}
