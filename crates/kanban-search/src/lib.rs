use kanban_core::TaskStatus;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub board: String,
    pub q: Option<String>,
    pub statuses: Vec<TaskStatus>,
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
                TantivyTaskIndexErrorKind::Unavailable | TantivyTaskIndexErrorKind::Corrupt
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

    #[derive(Clone, Copy)]
    struct Fields {
        board_id: Field,
        task_id: Field,
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
        fs::create_dir_all(parent)?;
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
        fs::write(
            tmp_path.join("kb-index-meta.json"),
            serde_json::to_vec_pretty(&metadata)?,
        )?;

        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        fs::rename(&tmp_path, path)?;
        Ok(metadata)
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

    pub fn search_task_index(
        path: &Path,
        board_id: &str,
        query: &SearchQuery,
        last_event_id: Option<i64>,
    ) -> Result<(Vec<SearchHit>, SearchMeta), TantivyTaskIndexError> {
        let metadata = read_task_index_metadata(path)?;
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
            (Some(current), Some(indexed)) => Some(current.saturating_sub(indexed)),
            (Some(current), None) => Some(current),
            _ => Some(0),
        };
        Ok((
            hits,
            SearchMeta {
                backend: "tantivy".to_owned(),
                stale: lag.is_some_and(|value| value > 0),
                index_version: Some(metadata.index_version),
                last_event_id: metadata.last_event_id,
                index_lag_events: lag,
            },
        ))
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

    fn to_tantivy_doc(fields: Fields, task: &TaskSearchDocument) -> TantivyDocument {
        let mut document = TantivyDocument::default();
        document.add_text(fields.board_id, &task.board_id);
        document.add_text(fields.task_id, &task.task_id);
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
