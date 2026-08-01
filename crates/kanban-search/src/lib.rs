use kanban_core::TaskStatus;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub board: String,
    pub q: Option<String>,
    pub statuses: Vec<TaskStatus>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub include_archived: bool,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub task_id: String,
    pub seq: i64,
    pub score: f64,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMeta {
    pub backend: String,
    pub stale: bool,
    pub database_instance_id: Option<String>,
    pub protocol_version: Option<i64>,
    pub generation: Option<String>,
    pub resolved_board_id: String,
    pub fallback_reason: Option<String>,
    pub index_version: Option<String>,
    pub last_event_id: Option<i64>,
    pub index_lag_events: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub meta: SearchMeta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndexStatus {
    pub backend: String,
    pub derived_index: bool,
    pub stale: bool,
    pub database_instance_id: Option<String>,
    pub protocol_version: Option<i64>,
    pub generation: Option<String>,
    pub resolved_board_id: String,
    pub fallback_reason: Option<String>,
    pub index_version: Option<String>,
    pub last_event_id: Option<i64>,
    pub index_lag_events: Option<i64>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSearchDocument {
    pub board_id: String,
    pub task_id: String,
    pub seq: i64,
    pub status: TaskStatus,
    pub assignee: Option<String>,
    pub priority: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub due_at: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub comments: String,
    pub run_text: String,
    pub event_text: String,
}

#[derive(Debug)]
pub struct SearchBackendError {
    message: String,
}

impl SearchBackendError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SearchBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for SearchBackendError {}

impl From<tantivy_backend::TantivyTaskIndexError> for SearchBackendError {
    fn from(value: tantivy_backend::TantivyTaskIndexError) -> Self {
        Self::new(value.to_string())
    }
}

#[cfg(feature = "tantivy-backend")]
pub mod tantivy_backend {
    use super::{SearchHit, SearchMeta, SearchQuery, TaskSearchDocument};
    use kanban_local::{
        durable_create_dir_all, durable_create_new_file, durable_publish_directory,
        durable_sync_directory_tree,
    };
    use serde::{Deserialize, Serialize};
    use std::{
        error::Error,
        fmt, fs,
        path::{Path, PathBuf},
    };
    use tantivy::{
        Index, TantivyDocument, Term,
        collector::TopDocs,
        query::{AllQuery, BooleanQuery, Occur, Query, QueryParser, QueryParserError, TermQuery},
        schema::{Field, IndexRecordOption, STORED, STRING, Schema, TEXT, Value},
    };

    pub const INDEX_VERSION: &str = "tasks-v1";
    pub const PROJECTION_INDEX_VERSION: &str = "tasks-v2";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TantivyTaskIndexErrorKind {
        Unavailable,
        Corrupt,
        Query,
        Schema,
        Io,
        Internal,
    }

    #[derive(Debug)]
    pub struct TantivyTaskIndexError {
        kind: TantivyTaskIndexErrorKind,
        message: String,
    }

    impl TantivyTaskIndexError {
        fn new(kind: TantivyTaskIndexErrorKind, message: impl Into<String>) -> Self {
            Self {
                kind,
                message: message.into(),
            }
        }

        pub fn kind(&self) -> TantivyTaskIndexErrorKind {
            self.kind
        }

        pub fn is_fallback_eligible(&self) -> bool {
            matches!(
                self.kind,
                TantivyTaskIndexErrorKind::Unavailable
                    | TantivyTaskIndexErrorKind::Corrupt
                    | TantivyTaskIndexErrorKind::Schema
            )
        }

        fn corrupt(error: impl fmt::Display) -> Self {
            Self::new(TantivyTaskIndexErrorKind::Corrupt, error.to_string())
        }
    }

    impl fmt::Display for TantivyTaskIndexError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.message)
        }
    }

    impl Error for TantivyTaskIndexError {}

    impl From<tantivy::TantivyError> for TantivyTaskIndexError {
        fn from(value: tantivy::TantivyError) -> Self {
            Self::new(TantivyTaskIndexErrorKind::Internal, value.to_string())
        }
    }

    impl From<std::io::Error> for TantivyTaskIndexError {
        fn from(value: std::io::Error) -> Self {
            Self::new(TantivyTaskIndexErrorKind::Io, value.to_string())
        }
    }

    impl From<serde_json::Error> for TantivyTaskIndexError {
        fn from(value: serde_json::Error) -> Self {
            Self::new(TantivyTaskIndexErrorKind::Corrupt, value.to_string())
        }
    }

    impl From<QueryParserError> for TantivyTaskIndexError {
        fn from(value: QueryParserError) -> Self {
            Self::new(TantivyTaskIndexErrorKind::Query, value.to_string())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TantivyIndexMetadata {
        pub index_version: String,
        pub board_id: String,
        pub last_event_id: Option<i64>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct TantivyTaskProjectionMetadata {
        pub database_instance_id: String,
        pub protocol_version: i64,
        pub schema_version: i64,
        pub generation: String,
        pub fence_epoch: i64,
        pub snapshot_cursor: i64,
        pub provider: String,
        pub provider_fingerprint: String,
        pub canonical_item_count: i64,
        pub canonical_digest: String,
        pub delivery_item_count: i64,
        pub delivery_digest: String,
        pub fingerprint: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TaskProjectionDocumentKey {
        pub board_id: String,
        pub task_id: String,
    }

    #[derive(Clone, Copy)]
    struct Fields {
        board_id: Field,
        task_id: Field,
        document_key: Option<Field>,
        seq: Field,
        status: Field,
        assignee: Field,
        priority: Field,
        created_at: Field,
        updated_at: Field,
        due_at: Field,
        title: Field,
        description: Field,
        comments: Field,
        run_text: Field,
        event_text: Field,
        aggregate_text: Field,
    }

    pub fn rebuild_task_index(
        path: &Path,
        board_id: &str,
        last_event_id: Option<i64>,
        documents: &[TaskSearchDocument],
    ) -> Result<TantivyIndexMetadata, TantivyTaskIndexError> {
        let parent = path.parent().ok_or_else(|| {
            TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Internal,
                "index path must have a parent",
            )
        })?;
        durable_create_dir_all(parent)?;
        let tmp_path = temp_index_path(path);
        if tmp_path.exists() {
            fs::remove_dir_all(&tmp_path)?;
        }
        fs::create_dir_all(&tmp_path)?;

        let (schema, fields) = task_schema();
        let index = Index::create_in_dir(&tmp_path, schema)?;
        let mut writer = index.writer(50_000_000)?;
        for document in documents {
            writer.add_document(to_tantivy_doc(fields, document))?;
        }
        writer.commit()?;

        let metadata = TantivyIndexMetadata {
            index_version: INDEX_VERSION.to_owned(),
            board_id: board_id.to_owned(),
            last_event_id,
        };
        write_index_metadata(&tmp_path, &metadata)?;

        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        fs::rename(&tmp_path, path)?;
        Ok(metadata)
    }

    pub fn sync_task_index(
        path: &Path,
        board_id: &str,
        last_event_id: Option<i64>,
        documents: &[TaskSearchDocument],
        delete_task_ids: &[String],
    ) -> Result<TantivyIndexMetadata, TantivyTaskIndexError> {
        let previous = validate_task_index(path, board_id)?;
        validate_metadata(&previous, board_id)?;
        let index = Index::open_in_dir(path).map_err(TantivyTaskIndexError::corrupt)?;
        let schema = index.schema();
        let fields = fields_from_schema(&schema)?;
        let mut writer = index.writer(50_000_000)?;
        for task_id in delete_task_ids {
            writer.delete_term(Term::from_field_text(fields.task_id, task_id));
        }
        for document in documents {
            writer.delete_term(Term::from_field_text(fields.task_id, &document.task_id));
            writer.add_document(to_tantivy_doc(fields, document))?;
        }
        writer.commit()?;

        let metadata = TantivyIndexMetadata {
            index_version: INDEX_VERSION.to_owned(),
            board_id: board_id.to_owned(),
            last_event_id,
        };
        write_index_metadata(path, &metadata)?;
        Ok(metadata)
    }

    pub fn prepare_task_projection_generation(
        path: &Path,
        metadata: &TantivyTaskProjectionMetadata,
        documents: &[TaskSearchDocument],
    ) -> Result<TantivyTaskProjectionMetadata, TantivyTaskIndexError> {
        validate_projection_metadata_shape(metadata)?;
        if path.exists() {
            let existing = validate_task_projection_generation(
                path,
                &metadata.database_instance_id,
                &metadata.generation,
            )?;
            if existing == *metadata {
                return Ok(existing);
            }
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Corrupt,
                "task projection generation already exists with different metadata",
            ));
        }
        let parent = path.parent().ok_or_else(|| {
            TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Internal,
                "projection generation path must have a parent",
            )
        })?;
        fs::create_dir_all(parent)?;
        let tmp_path = temp_index_path(path);
        if tmp_path.exists() {
            fs::remove_dir_all(&tmp_path)?;
        }
        fs::create_dir_all(&tmp_path)?;
        let result = (|| {
            let (schema, fields) = projection_task_schema();
            let index = Index::create_in_dir(&tmp_path, schema)?;
            let mut writer = index.writer(50_000_000)?;
            for document in documents {
                writer.add_document(to_tantivy_doc(fields, document))?;
            }
            writer.commit()?;
            // Tantivy keeps Windows file handles for the index directory and
            // its writer until both values are dropped.  The generation
            // publish below renames the complete directory, which Windows
            // rejects while any descendant handle is still open.  Keep the
            // handles alive through commit, then close them before metadata
            // creation and the durable directory rename.
            drop(writer);
            drop(index);
            write_projection_metadata(&tmp_path, metadata)?;
            durable_publish_directory(&tmp_path, path)?;
            Ok(metadata.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&tmp_path);
        }
        result
    }

    pub fn sync_task_projection_generation(
        path: &Path,
        expected: &TantivyTaskProjectionMetadata,
        documents: &[TaskSearchDocument],
        delete_keys: &[TaskProjectionDocumentKey],
    ) -> Result<TantivyTaskProjectionMetadata, TantivyTaskIndexError> {
        let metadata = validate_task_projection_generation(
            path,
            &expected.database_instance_id,
            &expected.generation,
        )?;
        if metadata != *expected {
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Corrupt,
                "task projection metadata changed during incremental update",
            ));
        }
        let index = Index::open_in_dir(path).map_err(TantivyTaskIndexError::corrupt)?;
        let fields = projection_fields_from_schema(&index.schema())?;
        let document_key = fields
            .document_key
            .expect("projection schema requires document_key");
        let mut writer = index.writer(50_000_000)?;
        for key in delete_keys {
            writer.delete_term(Term::from_field_text(
                document_key,
                &projection_document_key(&key.board_id, &key.task_id),
            ));
        }
        for document in documents {
            writer.delete_term(Term::from_field_text(
                document_key,
                &projection_document_key(&document.board_id, &document.task_id),
            ));
            writer.add_document(to_tantivy_doc(fields, document))?;
        }
        writer.commit()?;
        drop(writer);
        durable_sync_directory_tree(path)?;
        Ok(metadata)
    }

    pub fn validate_task_projection_generation(
        path: &Path,
        database_instance_id: &str,
        generation: &str,
    ) -> Result<TantivyTaskProjectionMetadata, TantivyTaskIndexError> {
        let metadata = read_projection_metadata(path)?;
        validate_projection_metadata_shape(&metadata)?;
        if metadata.database_instance_id != database_instance_id {
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Unavailable,
                "task projection database identity mismatch",
            ));
        }
        if metadata.generation != generation {
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Unavailable,
                "task projection generation mismatch",
            ));
        }
        let index = Index::open_in_dir(path).map_err(TantivyTaskIndexError::corrupt)?;
        projection_fields_from_schema(&index.schema())?;
        index.reader().map_err(TantivyTaskIndexError::corrupt)?;
        Ok(metadata)
    }

    pub fn search_task_projection_generation(
        path: &Path,
        database_instance_id: &str,
        generation: &str,
        query: &SearchQuery,
    ) -> Result<(Vec<SearchHit>, SearchMeta), TantivyTaskIndexError> {
        let metadata = validate_task_projection_generation(path, database_instance_id, generation)?;
        let index = Index::open_in_dir(path).map_err(TantivyTaskIndexError::corrupt)?;
        let fields = projection_fields_from_schema(&index.schema())?;
        let hits = search_index(&index, fields, &query.board, query)?;
        Ok((
            hits,
            SearchMeta {
                backend: "tantivy".to_owned(),
                stale: false,
                database_instance_id: Some(database_instance_id.to_owned()),
                protocol_version: Some(metadata.protocol_version),
                generation: Some(generation.to_owned()),
                resolved_board_id: query.board.clone(),
                fallback_reason: None,
                index_version: Some(PROJECTION_INDEX_VERSION.to_owned()),
                last_event_id: None,
                index_lag_events: None,
            },
        ))
    }

    pub fn search_task_projection_generation_against(
        path: &Path,
        expected: &TantivyTaskProjectionMetadata,
        query: &SearchQuery,
    ) -> Result<(Vec<SearchHit>, SearchMeta), TantivyTaskIndexError> {
        let metadata = validate_task_projection_generation(
            path,
            &expected.database_instance_id,
            &expected.generation,
        )?;
        if metadata != *expected {
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Corrupt,
                "task projection metadata does not match SQLite active evidence",
            ));
        }
        search_task_projection_generation(
            path,
            &expected.database_instance_id,
            &expected.generation,
            query,
        )
    }

    pub fn task_index_exists(path: &Path) -> bool {
        path.join("meta.json").exists() || path.join("kb-index-meta.json").exists()
    }

    pub fn read_task_index_metadata(
        path: &Path,
    ) -> Result<TantivyIndexMetadata, TantivyTaskIndexError> {
        let metadata = fs::read(path.join("kb-index-meta.json")).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                TantivyTaskIndexError::new(
                    TantivyTaskIndexErrorKind::Unavailable,
                    "task index metadata is missing",
                )
            } else {
                TantivyTaskIndexError::new(TantivyTaskIndexErrorKind::Unavailable, err.to_string())
            }
        })?;
        serde_json::from_slice(&metadata).map_err(TantivyTaskIndexError::from)
    }

    pub fn validate_task_index(
        path: &Path,
        board_id: &str,
    ) -> Result<TantivyIndexMetadata, TantivyTaskIndexError> {
        let metadata = read_task_index_metadata(path)?;
        validate_metadata(&metadata, board_id)?;
        let index = Index::open_in_dir(path).map_err(TantivyTaskIndexError::corrupt)?;
        let schema = index.schema();
        fields_from_schema(&schema)?;
        index.reader().map_err(TantivyTaskIndexError::corrupt)?;
        Ok(metadata)
    }

    pub fn search_task_index(
        path: &Path,
        board_id: &str,
        query: &SearchQuery,
        last_event_id: Option<i64>,
    ) -> Result<(Vec<SearchHit>, SearchMeta), TantivyTaskIndexError> {
        let metadata = validate_task_index(path, board_id)?;
        let index = Index::open_in_dir(path).map_err(TantivyTaskIndexError::corrupt)?;
        let schema = index.schema();
        let fields = fields_from_schema(&schema)?;
        let reader = index.reader().map_err(TantivyTaskIndexError::corrupt)?;
        let searcher = reader.searcher();
        let search_query = build_query(&index, fields, board_id, query)?;
        let wanted = query.limit.checked_add(query.offset).ok_or_else(|| {
            TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Query,
                "search page bound overflow",
            )
        })?;
        let top_docs = searcher
            .search(
                &*search_query,
                &TopDocs::with_limit(wanted).order_by_score(),
            )
            .map_err(TantivyTaskIndexError::corrupt)?;
        let mut hits = Vec::new();
        for (score, address) in top_docs.into_iter().skip(query.offset) {
            let document = searcher
                .doc::<TantivyDocument>(address)
                .map_err(TantivyTaskIndexError::corrupt)?;
            let task_id = text_value(&document, fields.task_id).ok_or_else(|| {
                TantivyTaskIndexError::new(
                    TantivyTaskIndexErrorKind::Internal,
                    "task_id missing from hit",
                )
            })?;
            let seq = i64_value(&document, fields.seq).unwrap_or_default();
            let snippet = query
                .q
                .as_deref()
                .and_then(|needle| snippet_from_document(&document, fields, needle));
            hits.push(SearchHit {
                task_id,
                seq,
                score: score.into(),
                snippet,
            });
        }
        let lag = match (last_event_id, metadata.last_event_id) {
            (Some(current), Some(indexed)) => {
                Some(current.abs_diff(indexed).try_into().unwrap_or(i64::MAX))
            }
            (Some(current), None) => Some(current),
            _ => Some(0),
        };
        Ok((
            hits,
            SearchMeta {
                backend: "tantivy".to_owned(),
                stale: lag.is_some_and(|value| value > 0),
                database_instance_id: None,
                protocol_version: None,
                generation: None,
                resolved_board_id: board_id.to_owned(),
                fallback_reason: None,
                index_version: Some(metadata.index_version),
                last_event_id: metadata.last_event_id,
                index_lag_events: lag,
            },
        ))
    }

    fn validate_metadata(
        metadata: &TantivyIndexMetadata,
        board_id: &str,
    ) -> Result<(), TantivyTaskIndexError> {
        if metadata.index_version != INDEX_VERSION {
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Corrupt,
                format!("unsupported task index version {}", metadata.index_version),
            ));
        }
        if metadata.board_id != board_id {
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Unavailable,
                "task index board mismatch",
            ));
        }
        Ok(())
    }

    fn write_index_metadata(
        path: &Path,
        metadata: &TantivyIndexMetadata,
    ) -> Result<(), TantivyTaskIndexError> {
        fs::write(
            path.join("kb-index-meta.json"),
            serde_json::to_vec_pretty(metadata)?,
        )?;
        Ok(())
    }

    fn read_projection_metadata(
        path: &Path,
    ) -> Result<TantivyTaskProjectionMetadata, TantivyTaskIndexError> {
        let bytes = fs::read(path.join("kb-projection-meta.json")).map_err(|error| {
            let kind = if error.kind() == std::io::ErrorKind::NotFound {
                TantivyTaskIndexErrorKind::Unavailable
            } else {
                TantivyTaskIndexErrorKind::Io
            };
            TantivyTaskIndexError::new(kind, error.to_string())
        })?;
        serde_json::from_slice(&bytes).map_err(TantivyTaskIndexError::from)
    }

    fn write_projection_metadata(
        path: &Path,
        metadata: &TantivyTaskProjectionMetadata,
    ) -> Result<(), TantivyTaskIndexError> {
        let metadata_path = path.join("kb-projection-meta.json");
        durable_create_new_file(&metadata_path, &serde_json::to_vec_pretty(metadata)?)?;
        Ok(())
    }

    pub fn sync_task_projection_generation_files(path: &Path) -> Result<(), TantivyTaskIndexError> {
        durable_sync_directory_tree(path)?;
        Ok(())
    }

    fn validate_projection_metadata_shape(
        metadata: &TantivyTaskProjectionMetadata,
    ) -> Result<(), TantivyTaskIndexError> {
        if metadata.protocol_version != 2
            || metadata.schema_version <= 0
            || metadata.fence_epoch < 0
            || metadata.snapshot_cursor < 0
            || metadata.database_instance_id.trim().is_empty()
            || metadata.generation.trim().is_empty()
            || metadata.provider.trim().is_empty()
            || metadata.provider_fingerprint.trim().is_empty()
            || metadata.canonical_item_count < 0
            || metadata.canonical_digest.trim().is_empty()
            || metadata.delivery_item_count < 0
            || metadata.delivery_digest.trim().is_empty()
            || metadata.fingerprint.trim().is_empty()
        {
            return Err(TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Corrupt,
                "invalid task projection metadata",
            ));
        }
        Ok(())
    }

    fn build_query(
        index: &Index,
        fields: Fields,
        board_id: &str,
        query: &SearchQuery,
    ) -> Result<Box<dyn Query>, TantivyTaskIndexError> {
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        clauses.push((
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(fields.board_id, board_id),
                IndexRecordOption::Basic,
            )),
        ));
        if !query.include_archived {
            clauses.push((
                Occur::MustNot,
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.status, "archived"),
                    IndexRecordOption::Basic,
                )),
            ));
        }
        if !query.statuses.is_empty() {
            let status_clauses = query
                .statuses
                .iter()
                .map(|status| {
                    (
                        Occur::Should,
                        Box::new(TermQuery::new(
                            Term::from_field_text(fields.status, status.as_str()),
                            IndexRecordOption::Basic,
                        )) as Box<dyn Query>,
                    )
                })
                .collect::<Vec<_>>();
            clauses.push((Occur::Must, Box::new(BooleanQuery::new(status_clauses))));
        }
        if let Some(assignee) = query
            .assignee
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            clauses.push((
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.assignee, assignee),
                    IndexRecordOption::Basic,
                )),
            ));
        }
        if let Some(q) = query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let parser = QueryParser::for_index(
                index,
                vec![
                    fields.title,
                    fields.description,
                    fields.comments,
                    fields.run_text,
                    fields.event_text,
                    fields.aggregate_text,
                ],
            );
            clauses.push((Occur::Must, parser.parse_query(q)?));
        } else {
            clauses.push((Occur::Must, Box::new(AllQuery)));
        }
        Ok(Box::new(BooleanQuery::new(clauses)))
    }

    fn search_index(
        index: &Index,
        fields: Fields,
        board_id: &str,
        query: &SearchQuery,
    ) -> Result<Vec<SearchHit>, TantivyTaskIndexError> {
        let reader = index.reader().map_err(TantivyTaskIndexError::corrupt)?;
        let searcher = reader.searcher();
        let search_query = build_query(index, fields, board_id, query)?;
        let wanted = query.limit.checked_add(query.offset).ok_or_else(|| {
            TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Query,
                "search page bound overflow",
            )
        })?;
        let top_docs = searcher
            .search(
                &*search_query,
                &TopDocs::with_limit(wanted).order_by_score(),
            )
            .map_err(TantivyTaskIndexError::corrupt)?;
        let mut hits = Vec::new();
        for (score, address) in top_docs.into_iter().skip(query.offset) {
            let document = searcher
                .doc::<TantivyDocument>(address)
                .map_err(TantivyTaskIndexError::corrupt)?;
            let task_id = text_value(&document, fields.task_id).ok_or_else(|| {
                TantivyTaskIndexError::new(
                    TantivyTaskIndexErrorKind::Internal,
                    "task_id missing from hit",
                )
            })?;
            let seq = i64_value(&document, fields.seq).unwrap_or_default();
            let snippet = query
                .q
                .as_deref()
                .and_then(|needle| snippet_from_document(&document, fields, needle));
            hits.push(SearchHit {
                task_id,
                seq,
                score: score.into(),
                snippet,
            });
        }
        Ok(hits)
    }

    fn task_schema() -> (Schema, Fields) {
        let mut builder = Schema::builder();
        let board_id = builder.add_text_field("board_id", STRING | STORED);
        let task_id = builder.add_text_field("task_id", STRING | STORED);
        let seq = builder.add_i64_field("seq", STORED);
        let status = builder.add_text_field("status", STRING | STORED);
        let assignee = builder.add_text_field("assignee", STRING | STORED);
        let priority = builder.add_i64_field("priority", STORED);
        let created_at = builder.add_i64_field("created_at", STORED);
        let updated_at = builder.add_i64_field("updated_at", STORED);
        let due_at = builder.add_i64_field("due_at", STORED);
        let title = builder.add_text_field("title", TEXT | STORED);
        let description = builder.add_text_field("description", TEXT | STORED);
        let comments = builder.add_text_field("comments", TEXT | STORED);
        let run_text = builder.add_text_field("run_text", TEXT | STORED);
        let event_text = builder.add_text_field("event_text", TEXT | STORED);
        let aggregate_text = builder.add_text_field("aggregate_text", TEXT | STORED);
        (
            builder.build(),
            Fields {
                board_id,
                task_id,
                document_key: None,
                seq,
                status,
                assignee,
                priority,
                created_at,
                updated_at,
                due_at,
                title,
                description,
                comments,
                run_text,
                event_text,
                aggregate_text,
            },
        )
    }

    fn projection_task_schema() -> (Schema, Fields) {
        let (schema, fields) = task_schema();
        let mut builder = Schema::builder();
        for (field, entry) in schema.fields() {
            let _ = field;
            builder.add_field(entry.clone());
        }
        let document_key = builder.add_text_field("document_key", STRING | STORED);
        (
            builder.build(),
            Fields {
                document_key: Some(document_key),
                ..fields
            },
        )
    }

    fn fields_from_schema(schema: &Schema) -> Result<Fields, TantivyTaskIndexError> {
        let field = |name: &str| {
            schema.get_field(name).map_err(|_| {
                TantivyTaskIndexError::new(
                    TantivyTaskIndexErrorKind::Schema,
                    format!("missing field {name}"),
                )
            })
        };
        Ok(Fields {
            board_id: field("board_id")?,
            task_id: field("task_id")?,
            document_key: None,
            seq: field("seq")?,
            status: field("status")?,
            assignee: field("assignee")?,
            priority: field("priority")?,
            created_at: field("created_at")?,
            updated_at: field("updated_at")?,
            due_at: field("due_at")?,
            title: field("title")?,
            description: field("description")?,
            comments: field("comments")?,
            run_text: field("run_text")?,
            event_text: field("event_text")?,
            aggregate_text: field("aggregate_text")?,
        })
    }

    fn projection_fields_from_schema(schema: &Schema) -> Result<Fields, TantivyTaskIndexError> {
        let mut fields = fields_from_schema(schema)?;
        fields.document_key = Some(schema.get_field("document_key").map_err(|_| {
            TantivyTaskIndexError::new(
                TantivyTaskIndexErrorKind::Schema,
                "missing field document_key",
            )
        })?);
        Ok(fields)
    }

    fn to_tantivy_doc(fields: Fields, task: &TaskSearchDocument) -> TantivyDocument {
        let mut document = TantivyDocument::default();
        document.add_text(fields.board_id, &task.board_id);
        document.add_text(fields.task_id, &task.task_id);
        if let Some(document_key) = fields.document_key {
            document.add_text(
                document_key,
                projection_document_key(&task.board_id, &task.task_id),
            );
        }
        document.add_i64(fields.seq, task.seq);
        document.add_text(fields.status, task.status.as_str());
        if let Some(assignee) = task.assignee.as_deref() {
            document.add_text(fields.assignee, assignee);
        }
        document.add_i64(fields.priority, task.priority);
        document.add_i64(fields.created_at, task.created_at);
        document.add_i64(fields.updated_at, task.updated_at);
        if let Some(due_at) = task.due_at {
            document.add_i64(fields.due_at, due_at);
        }
        document.add_text(fields.title, &task.title);
        if let Some(description) = task.description.as_deref() {
            document.add_text(fields.description, description);
        }
        document.add_text(fields.comments, &task.comments);
        document.add_text(fields.run_text, &task.run_text);
        document.add_text(fields.event_text, &task.event_text);
        document.add_text(
            fields.aggregate_text,
            [
                task.title.as_str(),
                task.description.as_deref().unwrap_or(""),
                task.comments.as_str(),
                task.run_text.as_str(),
                task.event_text.as_str(),
            ]
            .join("\n"),
        );
        document
    }

    fn projection_document_key(board_id: &str, task_id: &str) -> String {
        format!("{}:{board_id}{}:{task_id}", board_id.len(), task_id.len())
    }

    fn text_value(document: &TantivyDocument, field: Field) -> Option<String> {
        document
            .get_first(field)
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
    }

    fn i64_value(document: &TantivyDocument, field: Field) -> Option<i64> {
        document.get_first(field).and_then(|value| value.as_i64())
    }

    fn snippet_from_document(
        document: &TantivyDocument,
        fields: Fields,
        needle: &str,
    ) -> Option<String> {
        let needle = needle.trim().to_lowercase();
        if needle.is_empty() {
            return None;
        }
        for field in [
            fields.title,
            fields.description,
            fields.comments,
            fields.run_text,
            fields.event_text,
        ] {
            if let Some(value) = text_value(document, field)
                && value.to_lowercase().contains(&needle)
            {
                return Some(trim_snippet(&value));
            }
        }
        text_value(document, fields.aggregate_text).map(|value| trim_snippet(&value))
    }

    fn trim_snippet(value: &str) -> String {
        let value = value.trim();
        if value.len() <= 240 {
            return value.to_owned();
        }
        let mut out = value.chars().take(240).collect::<String>();
        out.push_str("...");
        out
    }

    fn temp_index_path(path: &Path) -> PathBuf {
        let mut tmp = path.to_path_buf();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("tasks");
        tmp.set_file_name(format!("{name}.tmp-{}", std::process::id()));
        tmp
    }
}

#[cfg(not(feature = "tantivy-backend"))]
pub mod tantivy_backend {
    #[derive(Debug)]
    pub struct TantivyTaskIndexError;

    impl std::fmt::Display for TantivyTaskIndexError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Tantivy backend feature is disabled")
        }
    }

    impl std::error::Error for TantivyTaskIndexError {}
}

#[cfg(all(test, feature = "tantivy-backend"))]
mod projection_v2_tests {
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use kanban_core::TaskStatus;

    use super::{
        SearchQuery, TaskSearchDocument,
        tantivy_backend::{
            TantivyTaskProjectionMetadata, TaskProjectionDocumentKey,
            prepare_task_projection_generation, search_task_projection_generation,
            sync_task_projection_generation, validate_task_projection_generation,
        },
    };

    static TEST_NONCE: AtomicU64 = AtomicU64::new(1);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "kanban-search-{name}-{}-{timestamp}-{}",
                std::process::id(),
                TEST_NONCE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn metadata(generation: &str) -> TantivyTaskProjectionMetadata {
        TantivyTaskProjectionMetadata {
            database_instance_id: "db_test".to_owned(),
            protocol_version: 2,
            schema_version: 1,
            generation: generation.to_owned(),
            fence_epoch: 7,
            snapshot_cursor: 11,
            provider: "tantivy".to_owned(),
            provider_fingerprint: "tantivy-tasks-v2".to_owned(),
            canonical_item_count: 2,
            canonical_digest: "fnv64:canonical".to_owned(),
            delivery_item_count: 2,
            delivery_digest: "fnv64:delivery".to_owned(),
            fingerprint: "fnv64:artifact".to_owned(),
        }
    }

    fn document(board_id: &str, task_id: &str, title: &str) -> TaskSearchDocument {
        TaskSearchDocument {
            board_id: board_id.to_owned(),
            task_id: task_id.to_owned(),
            seq: 1,
            status: TaskStatus::Ready,
            assignee: None,
            priority: 1,
            created_at: 1,
            updated_at: 1,
            due_at: None,
            title: title.to_owned(),
            description: None,
            comments: String::new(),
            run_text: String::new(),
            event_text: String::new(),
        }
    }

    fn query(board: &str, text: &str) -> SearchQuery {
        SearchQuery {
            board: board.to_owned(),
            q: Some(text.to_owned()),
            statuses: Vec::new(),
            labels: Vec::new(),
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        }
    }

    #[test]
    fn projection_generation_contains_multiple_boards_and_forces_board_scope() {
        let temp = TestDir::new("multi-board");
        let path = temp.path().join("generation");
        let documents = vec![
            document("board-a", "task-a", "shared needle"),
            document("board-b", "task-b", "shared needle"),
        ];

        prepare_task_projection_generation(&path, &metadata("gen-a"), &documents).unwrap();

        let (board_a, _) = search_task_projection_generation(
            &path,
            "db_test",
            "gen-a",
            &query("board-a", "needle"),
        )
        .unwrap();
        let (board_b, _) = search_task_projection_generation(
            &path,
            "db_test",
            "gen-a",
            &query("board-b", "needle"),
        )
        .unwrap();
        assert_eq!(
            board_a
                .iter()
                .map(|hit| hit.task_id.as_str())
                .collect::<Vec<_>>(),
            ["task-a"]
        );
        assert_eq!(
            board_b
                .iter()
                .map(|hit| hit.task_id.as_str())
                .collect::<Vec<_>>(),
            ["task-b"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn projection_prepare_closes_tantivy_handles_before_generation_publish() {
        let temp = TestDir::new("windows-publish-handle-lifetime");
        let path = temp.path().join("generation");

        prepare_task_projection_generation(
            &path,
            &metadata("gen-windows"),
            &[document("board-a", "task-a", "handle lifetime")],
        )
        .expect("a prepared generation must publish after Tantivy handles close");

        // Recovery moves whole generations by rename.  Keeping this
        // operation in the backend test makes the Windows handle-lifetime
        // boundary explicit without bypassing service authority checks.
        let moved = temp.path().join("generation.moved");
        std::fs::rename(&path, &moved).expect("published generation must be movable");
        std::fs::rename(&moved, &path).expect("generation must be restorable");
    }

    #[test]
    fn projection_sync_delete_uses_board_and_task_composite_key() {
        let temp = TestDir::new("composite-delete");
        let path = temp.path().join("generation");
        let documents = vec![
            document("board-a", "shared-task", "alpha"),
            document("board-b", "shared-task", "beta"),
        ];
        let manifest = metadata("gen-a");
        prepare_task_projection_generation(&path, &manifest, &documents).unwrap();

        sync_task_projection_generation(
            &path,
            &manifest,
            &[],
            &[TaskProjectionDocumentKey {
                board_id: "board-a".to_owned(),
                task_id: "shared-task".to_owned(),
            }],
        )
        .unwrap();

        let (board_a, _) = search_task_projection_generation(
            &path,
            "db_test",
            "gen-a",
            &query("board-a", "alpha"),
        )
        .unwrap();
        let (board_b, _) =
            search_task_projection_generation(&path, "db_test", "gen-a", &query("board-b", "beta"))
                .unwrap();
        assert!(board_a.is_empty());
        assert_eq!(board_b.len(), 1);
    }

    #[test]
    fn projection_metadata_fails_closed_for_database_or_generation_mismatch() {
        let temp = TestDir::new("metadata-mismatch");
        let path = temp.path().join("generation");
        prepare_task_projection_generation(
            &path,
            &metadata("gen-a"),
            &[document("board-a", "task-a", "needle")],
        )
        .unwrap();

        assert!(validate_task_projection_generation(&path, "db_other", "gen-a").is_err());
        assert!(validate_task_projection_generation(&path, "db_test", "gen-other").is_err());
        assert!(
            search_task_projection_generation(
                Path::new(&path),
                "db_other",
                "gen-a",
                &query("board-a", "needle"),
            )
            .is_err()
        );
    }
}
