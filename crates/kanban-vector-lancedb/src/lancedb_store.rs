use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::Array;
use arrow_array::types::Float32Type;
use arrow_array::{
    FixedSizeListArray, Float32Array, Int64Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use futures::TryStreamExt;
use lancedb::Table;
use lancedb::database::CreateTableMode;
use lancedb::expr::{col, lit};
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use tokio::runtime::Runtime;

use crate::LanceDbConfig;
use kanban_vector::{
    ChunkVectorStore, EmbeddingChunk, EmbeddingProvider, LabelAtomHit, LabelAtomQuery,
    LabelAtomVector, LabelAtomVectorHit, LabelAtomVectorQuery, LabelAtomVectorStore,
    QueryEmbeddingProvider, VectorError, VectorHit, VectorQuery, VectorStoreBackend,
    VectorStoreStatus, ensure_dimensions, normalize_semantic_text, semantic_content_hash,
};

const VECTOR_COLUMN: &str = "vector";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProjectionContentRow {
    pub key: String,
    pub content_json: String,
    pub vector_bits: Option<Vec<u32>>,
}

pub struct LanceDbStore {
    config: LanceDbConfig,
    provider: Option<Arc<dyn EmbeddingProvider + Send + Sync>>,
    runtime: Runtime,
}

/// Existing-table-only projection reader used by validation and recovery.
///
/// This type exposes no mutation traits and its table accessor never calls an
/// `ensure_*`/create path. Missing or incomplete historical tables are errors.
pub(crate) struct LanceDbProjectionReader {
    config: LanceDbConfig,
    dimensions: usize,
    runtime: Runtime,
}

impl LanceDbStore {
    pub fn connect(config: LanceDbConfig) -> Result<Self, VectorError> {
        config.execution_policy.validate()?;
        let provider = config.provider.clone();
        let runtime = Runtime::new().map_err(|err| VectorError::Store(err.to_string()))?;
        if let Some(provider) = provider.as_ref() {
            let table_name = config.table_name.clone();
            let path = path_string(&config)?;
            let dimensions = provider.dimensions();
            runtime.block_on(async {
                let connection = lancedb::connect(&path)
                    .execute()
                    .await
                    .map_err(map_lancedb_error)?;
                ensure_table(&connection, &table_name, dimensions).await?;
                Ok::<(), VectorError>(())
            })?;
        }
        Ok(Self {
            config,
            provider,
            runtime,
        })
    }

    fn provider(&self) -> Result<&Arc<dyn EmbeddingProvider + Send + Sync>, VectorError> {
        self.provider
            .as_ref()
            .ok_or(VectorError::MissingEmbeddingProvider)
    }

    fn table(&self, dimensions: usize) -> Result<Table, VectorError> {
        let table_name = self.config.table_name.clone();
        let path = path_string(&self.config)?;
        self.runtime.block_on(async {
            let connection = lancedb::connect(&path)
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            ensure_table(&connection, &table_name, dimensions).await
        })
    }

    fn label_atom_table(&self, dimensions: usize) -> Result<Table, VectorError> {
        let table_name = self.config.label_atom_table_name.clone();
        let path = path_string(&self.config)?;
        self.runtime.block_on(async {
            let connection = lancedb::connect(&path)
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            ensure_label_atom_table(&connection, &table_name, dimensions).await
        })
    }

    pub(crate) fn ensure_label_atom_projection_table(&self) -> Result<(), VectorError> {
        let provider = self.provider()?;
        self.label_atom_table(provider.dimensions()).map(|_| ())
    }

    pub(crate) fn chunk_projection_content_rows(
        &self,
    ) -> Result<Vec<ProjectionContentRow>, VectorError> {
        let provider = self.provider()?;
        let table = self.table(provider.dimensions())?;
        let batches = self.runtime.block_on(async {
            table
                .query()
                .select(Select::columns(&[
                    "chunk_key",
                    "entity_uri",
                    "chunk_uri",
                    "kind",
                    "project_id",
                    "board_id",
                    "task_id",
                    "source_table",
                    "source_id",
                    "text",
                    "summary",
                    "embedding_model",
                    "content_hash",
                    "created_at",
                    "updated_at",
                    "source_event_id",
                    "metadata_json",
                    "ordinal",
                    VECTOR_COLUMN,
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)
        })?;
        chunk_batches_to_projection_rows(&batches)
    }

    pub(crate) fn label_atom_projection_content_rows(
        &self,
    ) -> Result<Vec<ProjectionContentRow>, VectorError> {
        let provider = self.provider()?;
        let table = self.label_atom_table(provider.dimensions())?;
        let batches = self.runtime.block_on(async {
            table
                .query()
                .select(Select::columns(&[
                    "atom_key",
                    "atom_id",
                    "label_id",
                    "label_name",
                    "board_id",
                    "polarity",
                    "kind",
                    "text",
                    "ordinal",
                    "content_hash",
                    "embedding_model",
                    "created_at",
                    "updated_at",
                    VECTOR_COLUMN,
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)
        })?;
        label_atom_batches_to_projection_rows(&batches)
    }
}

impl LanceDbProjectionReader {
    pub(crate) fn open_existing(
        path: impl Into<std::path::PathBuf>,
        dimensions: usize,
    ) -> Result<Self, VectorError> {
        if dimensions == 0 {
            return Err(VectorError::Store(
                "historical projection dimensions must be non-zero".to_owned(),
            ));
        }
        Ok(Self {
            config: LanceDbConfig::degraded(path),
            dimensions,
            runtime: Runtime::new().map_err(|error| VectorError::Store(error.to_string()))?,
        })
    }

    fn table(&self, table_name: &str) -> Result<Table, VectorError> {
        let path = path_string(&self.config)?;
        self.runtime.block_on(async {
            let connection = lancedb::connect(&path)
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            open_existing_table(&connection, table_name, self.dimensions).await
        })
    }

    pub(crate) fn validate_chunk_projection_table(&self) -> Result<(), VectorError> {
        self.table(&self.config.table_name).map(|_| ())
    }

    pub(crate) fn validate_label_atom_projection_table(&self) -> Result<(), VectorError> {
        self.table(&self.config.label_atom_table_name).map(|_| ())
    }

    pub(crate) fn chunk_projection_content_rows(
        &self,
    ) -> Result<Vec<ProjectionContentRow>, VectorError> {
        let table = self.table(&self.config.table_name)?;
        let batches = self.runtime.block_on(async {
            table
                .query()
                .select(Select::columns(&[
                    "chunk_key",
                    "entity_uri",
                    "chunk_uri",
                    "kind",
                    "project_id",
                    "board_id",
                    "task_id",
                    "source_table",
                    "source_id",
                    "text",
                    "summary",
                    "embedding_model",
                    "content_hash",
                    "created_at",
                    "updated_at",
                    "source_event_id",
                    "metadata_json",
                    "ordinal",
                    VECTOR_COLUMN,
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)
        })?;
        chunk_batches_to_projection_rows(&batches)
    }

    pub(crate) fn label_atom_projection_content_rows(
        &self,
    ) -> Result<Vec<ProjectionContentRow>, VectorError> {
        let table = self.table(&self.config.label_atom_table_name)?;
        let batches = self.runtime.block_on(async {
            table
                .query()
                .select(Select::columns(&[
                    "atom_key",
                    "atom_id",
                    "label_id",
                    "label_name",
                    "board_id",
                    "polarity",
                    "kind",
                    "text",
                    "ordinal",
                    "content_hash",
                    "embedding_model",
                    "created_at",
                    "updated_at",
                    VECTOR_COLUMN,
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)
        })?;
        label_atom_batches_to_projection_rows(&batches)
    }

    pub(crate) fn query_chunks(
        &self,
        provider: &(dyn EmbeddingProvider + Send + Sync),
        query: &VectorQuery,
    ) -> Result<Vec<VectorHit>, VectorError> {
        if query.limit == 0 {
            return Ok(Vec::new());
        }
        let embedding = provider.embed(&query.text)?;
        ensure_dimensions(&embedding, self.dimensions)?;
        let table = self.table(&self.config.table_name)?;
        let provider_model = provider.embedding_model().to_owned();
        let board_id = query.board_id.clone();
        self.runtime.block_on(async {
            let filter = col("embedding_model")
                .eq(lit(provider_model))
                .and(col("board_id").eq(lit(board_id)));
            let stream = table
                .query()
                .nearest_to(embedding)
                .map_err(map_lancedb_error)?
                .column(VECTOR_COLUMN)
                .only_if_expr(filter)
                .limit(query.limit)
                .select(Select::columns(&[
                    "chunk_uri",
                    "entity_uri",
                    "ordinal",
                    "content_hash",
                    "text",
                    "summary",
                    "_distance",
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let batches = stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)?;
            batches_to_hits(&batches)
        })
    }

    pub(crate) fn query_label_atoms(
        &self,
        provider: &(dyn EmbeddingProvider + Send + Sync),
        query: &LabelAtomQuery,
    ) -> Result<Vec<LabelAtomHit>, VectorError> {
        if query.limit == 0 {
            return Ok(Vec::new());
        }
        let embedding_model = query
            .embedding_model
            .as_deref()
            .unwrap_or(provider.embedding_model());
        if embedding_model != provider.embedding_model() {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: provider.embedding_model().to_owned(),
                actual: embedding_model.to_owned(),
            });
        }
        let embedding = provider.embed(&query.text)?;
        ensure_dimensions(&embedding, self.dimensions)?;
        let table = self.table(&self.config.label_atom_table_name)?;
        let embedding_model = embedding_model.to_owned();
        let board_id = query.board_id.clone();
        let polarity = query.polarity.clone();
        self.runtime.block_on(async {
            let mut filter = col("embedding_model").eq(lit(embedding_model));
            if let Some(board_id) = board_id {
                filter = filter.and(col("board_id").eq(lit(board_id)));
            }
            if let Some(polarity) = polarity {
                filter = filter.and(col("polarity").eq(lit(polarity)));
            }
            let stream = table
                .query()
                .nearest_to(embedding)
                .map_err(map_lancedb_error)?
                .column(VECTOR_COLUMN)
                .only_if_expr(filter)
                .limit(query.limit)
                .select(Select::columns(&[
                    "atom_id",
                    "label_id",
                    "label_name",
                    "board_id",
                    "polarity",
                    "kind",
                    "text",
                    "ordinal",
                    "content_hash",
                    "embedding_model",
                    "_distance",
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let batches = stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)?;
            batches_to_label_atom_hits(&batches)
        })
    }

    pub(crate) fn query_label_atoms_by_vector(
        &self,
        provider: &(dyn EmbeddingProvider + Send + Sync),
        query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        if query.limit == 0 {
            return Ok(Vec::new());
        }
        let embedding_model = query
            .embedding_model
            .as_deref()
            .unwrap_or(provider.embedding_model());
        if embedding_model != provider.embedding_model() {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: provider.embedding_model().to_owned(),
                actual: embedding_model.to_owned(),
            });
        }
        ensure_dimensions(&query.vector, self.dimensions)?;
        let table = self.table(&self.config.label_atom_table_name)?;
        let embedding_model = embedding_model.to_owned();
        let board_id = query.board_id.clone();
        let polarity = query.polarity.clone();
        let vector = query.vector.clone();
        let mut columns = vec![
            "atom_id",
            "label_id",
            "label_name",
            "board_id",
            "polarity",
            "kind",
            "text",
            "ordinal",
            "content_hash",
            "embedding_model",
            "_distance",
        ];
        if query.include_vector {
            columns.push(VECTOR_COLUMN);
        }
        self.runtime.block_on(async {
            let mut filter = col("embedding_model").eq(lit(embedding_model));
            if let Some(board_id) = board_id {
                filter = filter.and(col("board_id").eq(lit(board_id)));
            }
            if let Some(polarity) = polarity {
                filter = filter.and(col("polarity").eq(lit(polarity)));
            }
            let stream = table
                .query()
                .nearest_to(vector)
                .map_err(map_lancedb_error)?
                .column(VECTOR_COLUMN)
                .only_if_expr(filter)
                .limit(query.limit)
                .select(Select::columns(&columns))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let batches = stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)?;
            batches_to_label_atom_vector_hits(&batches, query.include_vector)
        })
    }
}

pub(crate) fn expected_chunk_projection_content_rows(
    chunks: &[EmbeddingChunk],
) -> Result<Vec<ProjectionContentRow>, VectorError> {
    normalize_projection_rows(
        chunks
            .iter()
            .map(|chunk| ProjectionContentRow {
                key: chunk.chunk_key(),
                content_json: serde_json::json!({
                    "chunk_key": chunk.chunk_key(),
                    "entity_uri": chunk.chunk.entity_uri.as_str(),
                    "chunk_uri": chunk.chunk.uri.as_str(),
                    "kind": &chunk.kind,
                    "project_id": &chunk.project_id,
                    "board_id": &chunk.board_id,
                    "task_id": &chunk.task_id,
                    "source_table": &chunk.source_table,
                    "source_id": &chunk.source_id,
                    "text": &chunk.text,
                    "summary": &chunk.summary,
                    "embedding_model": &chunk.embedding_model,
                    "content_hash": &chunk.chunk.content_hash,
                    "created_at": chunk.created_at,
                    "updated_at": chunk.updated_at,
                    "source_event_id": chunk.source_event_id,
                    "metadata_json": &chunk.metadata_json,
                    "ordinal": chunk.chunk.ordinal,
                })
                .to_string(),
                vector_bits: None,
            })
            .collect(),
    )
}

pub(crate) fn expected_label_atom_projection_content_rows(
    atoms: &[LabelAtomVector],
) -> Result<Vec<ProjectionContentRow>, VectorError> {
    normalize_projection_rows(
        atoms
            .iter()
            .map(|atom| ProjectionContentRow {
                key: atom.atom_key(),
                content_json: serde_json::json!({
                    "atom_key": atom.atom_key(),
                    "atom_id": &atom.atom_id,
                    "label_id": &atom.label_id,
                    "label_name": &atom.label_name,
                    "board_id": &atom.board_id,
                    "polarity": &atom.polarity,
                    "kind": &atom.kind,
                    "text": &atom.text,
                    "ordinal": atom.ordinal,
                    "content_hash": &atom.content_hash,
                    "embedding_model": &atom.embedding_model,
                    "created_at": atom.created_at,
                    "updated_at": atom.updated_at,
                })
                .to_string(),
                vector_bits: None,
            })
            .collect(),
    )
}

impl VectorStoreBackend for LanceDbStore {
    fn embedding_model(&self) -> &str {
        self.provider
            .as_ref()
            .map(|provider| provider.embedding_model())
            .unwrap_or(kanban_vector::DEFAULT_EMBEDDING_MODEL)
    }

    fn status(&self) -> VectorStoreStatus {
        match self.provider.as_ref() {
            Some(provider) => VectorStoreStatus::new(
                "lancedb",
                true,
                format!(
                    "LanceDB vector store enabled for model {} ({} dimensions)",
                    provider.embedding_model(),
                    provider.dimensions()
                ),
            ),
            None => VectorStoreStatus::new(
                "lancedb",
                false,
                "LanceDB configured without an embedding provider; vector retrieval degraded",
            ),
        }
    }
}

impl ChunkVectorStore for LanceDbStore {
    fn upsert(&self, chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        let provider = self.provider()?;
        if chunks.is_empty() {
            return Ok(());
        }

        let expected_model = provider.embedding_model();
        if let Some(chunk) = chunks
            .iter()
            .find(|chunk| chunk.embedding_model != expected_model)
        {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: expected_model.to_owned(),
                actual: chunk.embedding_model.clone(),
            });
        }
        if chunks
            .iter()
            .any(|chunk| chunk.board_id.as_deref().is_none_or(str::is_empty))
        {
            return Err(VectorError::Store(
                "LanceDB task chunks require a non-empty board_id".to_owned(),
            ));
        }

        let dimensions = provider.dimensions();
        let embeddings = embed_deduplicated(
            provider.as_ref(),
            chunks.iter().map(|chunk| chunk.text.as_str()),
            &self.config.execution_policy,
        )?;

        let table = self.table(dimensions)?;
        let batch = chunks_to_batch(chunks, &embeddings, dimensions)?;
        let schema = batch.schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        self.runtime.block_on(async {
            let mut merge = table.merge_insert(&["chunk_key"]);
            merge.when_matched_update_all(None);
            merge.when_not_matched_insert_all();
            merge
                .execute(Box::new(reader))
                .await
                .map_err(map_lancedb_error)?;
            Ok(())
        })
    }

    fn delete_board(&self, board_id: &str) -> Result<(), VectorError> {
        let provider = self.provider()?;
        let embedding_model = provider.embedding_model().to_owned();
        let table = self.table(provider.dimensions())?;
        self.runtime.block_on(async {
            let predicate = col("board_id")
                .eq(lit(board_id.to_owned()))
                .and(col("embedding_model").eq(lit(embedding_model)));
            table.delete(&predicate).await.map_err(map_lancedb_error)?;
            Ok(())
        })
    }

    fn delete_entities(&self, entity_uris: &[String]) -> Result<(), VectorError> {
        let provider = self.provider()?;
        if entity_uris.is_empty() {
            return Ok(());
        }

        let mut entity_uris = entity_uris.to_vec();
        entity_uris.sort();
        entity_uris.dedup();
        let embedding_model = provider.embedding_model().to_owned();
        let table = self.table(provider.dimensions())?;
        self.runtime.block_on(async {
            for entity_uri in entity_uris {
                let predicate = col("entity_uri")
                    .eq(lit(entity_uri))
                    .and(col("embedding_model").eq(lit(embedding_model.clone())));
                table.delete(&predicate).await.map_err(map_lancedb_error)?;
            }
            Ok(())
        })
    }

    fn query(&self, query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        if query.limit == 0 {
            return Ok(Vec::new());
        }

        let provider = self.provider()?;
        let embedding = provider.embed(&query.text)?;
        ensure_dimensions(&embedding, provider.dimensions())?;
        let table_name = self.config.table_name.clone();
        let path = path_string(&self.config)?;
        self.runtime.block_on(async {
            let connection = lancedb::connect(&path)
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let table = match connection.open_table(&table_name).execute().await {
                Ok(table) => table,
                Err(lancedb::Error::TableNotFound { .. }) => return Ok(Vec::new()),
                Err(err) => return Err(map_lancedb_error(err)),
            };
            validate_vector_schema(&table, provider.dimensions()).await?;
            let filter = col("embedding_model")
                .eq(lit(provider.embedding_model()))
                .and(col("board_id").eq(lit(query.board_id.clone())));
            let stream = table
                .query()
                .nearest_to(embedding)
                .map_err(map_lancedb_error)?
                .column(VECTOR_COLUMN)
                .only_if_expr(filter)
                .limit(query.limit)
                .select(Select::columns(&[
                    "chunk_uri",
                    "entity_uri",
                    "ordinal",
                    "content_hash",
                    "text",
                    "summary",
                    "_distance",
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let batches = stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)?;
            batches_to_hits(&batches)
        })
    }
}

impl QueryEmbeddingProvider for LanceDbStore {
    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        let provider = self.provider()?;
        let embedding = provider.embed(text)?;
        ensure_dimensions(&embedding, provider.dimensions())?;
        Ok(embedding)
    }
}

impl LabelAtomVectorStore for LanceDbStore {
    fn delete_label_atoms_for_board(&self, board_id: &str) -> Result<(), VectorError> {
        let provider = self.provider()?;
        let embedding_model = provider.embedding_model().to_owned();
        let table = self.label_atom_table(provider.dimensions())?;
        self.runtime.block_on(async {
            let predicate = col("board_id")
                .eq(lit(board_id.to_owned()))
                .and(col("embedding_model").eq(lit(embedding_model)));
            table.delete(&predicate).await.map_err(map_lancedb_error)?;
            Ok(())
        })
    }

    fn upsert_label_atoms(&self, atoms: &[LabelAtomVector]) -> Result<(), VectorError> {
        let provider = self.provider()?;
        if atoms.is_empty() {
            return Ok(());
        }

        let expected_model = provider.embedding_model();
        if let Some(atom) = atoms
            .iter()
            .find(|atom| atom.embedding_model != expected_model)
        {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: expected_model.to_owned(),
                actual: atom.embedding_model.clone(),
            });
        }

        let dimensions = provider.dimensions();
        let embeddings = embed_deduplicated(
            provider.as_ref(),
            atoms.iter().map(|atom| atom.text.as_str()),
            &self.config.execution_policy,
        )?;

        let table = self.label_atom_table(dimensions)?;
        let batch = label_atoms_to_batch(atoms, &embeddings, dimensions)?;
        let schema = batch.schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        self.runtime.block_on(async {
            let mut merge = table.merge_insert(&["atom_key"]);
            merge.when_matched_update_all(None);
            merge.when_not_matched_insert_all();
            merge
                .execute(Box::new(reader))
                .await
                .map_err(map_lancedb_error)?;
            Ok(())
        })
    }

    fn query_label_atoms(&self, query: &LabelAtomQuery) -> Result<Vec<LabelAtomHit>, VectorError> {
        if query.limit == 0 {
            return Ok(Vec::new());
        }

        let provider = self.provider()?;
        let embedding_model = query
            .embedding_model
            .as_deref()
            .unwrap_or(provider.embedding_model());
        if embedding_model != provider.embedding_model() {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: provider.embedding_model().to_owned(),
                actual: embedding_model.to_owned(),
            });
        }

        let embedding = provider.embed(&query.text)?;
        ensure_dimensions(&embedding, provider.dimensions())?;
        let table_name = self.config.label_atom_table_name.clone();
        let path = path_string(&self.config)?;
        self.runtime.block_on(async {
            let connection = lancedb::connect(&path)
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let table = match connection.open_table(&table_name).execute().await {
                Ok(table) => table,
                Err(lancedb::Error::TableNotFound { .. }) => return Ok(Vec::new()),
                Err(err) => return Err(map_lancedb_error(err)),
            };
            validate_vector_schema(&table, provider.dimensions()).await?;
            let mut filter = col("embedding_model").eq(lit(embedding_model.to_owned()));
            if let Some(board_id) = &query.board_id {
                filter = filter.and(col("board_id").eq(lit(board_id.clone())));
            }
            if let Some(polarity) = &query.polarity {
                filter = filter.and(col("polarity").eq(lit(polarity.clone())));
            }
            let stream = table
                .query()
                .nearest_to(embedding)
                .map_err(map_lancedb_error)?
                .column(VECTOR_COLUMN)
                .only_if_expr(filter)
                .limit(query.limit)
                .select(Select::columns(&[
                    "atom_id",
                    "label_id",
                    "label_name",
                    "board_id",
                    "polarity",
                    "kind",
                    "text",
                    "ordinal",
                    "content_hash",
                    "embedding_model",
                    "_distance",
                ]))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let batches = stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)?;
            batches_to_label_atom_hits(&batches)
        })
    }

    fn query_label_atoms_by_vector(
        &self,
        query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        if query.limit == 0 {
            return Ok(Vec::new());
        }

        let provider = self.provider()?;
        let embedding_model = query
            .embedding_model
            .as_deref()
            .unwrap_or(provider.embedding_model());
        if embedding_model != provider.embedding_model() {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: provider.embedding_model().to_owned(),
                actual: embedding_model.to_owned(),
            });
        }

        ensure_dimensions(&query.vector, provider.dimensions())?;
        let table_name = self.config.label_atom_table_name.clone();
        let path = path_string(&self.config)?;
        self.runtime.block_on(async {
            let connection = lancedb::connect(&path)
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let table = match connection.open_table(&table_name).execute().await {
                Ok(table) => table,
                Err(lancedb::Error::TableNotFound { .. }) => return Ok(Vec::new()),
                Err(err) => return Err(map_lancedb_error(err)),
            };
            validate_vector_schema(&table, provider.dimensions()).await?;
            let mut filter = col("embedding_model").eq(lit(embedding_model.to_owned()));
            if let Some(board_id) = &query.board_id {
                filter = filter.and(col("board_id").eq(lit(board_id.clone())));
            }
            if let Some(polarity) = &query.polarity {
                filter = filter.and(col("polarity").eq(lit(polarity.clone())));
            }
            let mut columns = vec![
                "atom_id",
                "label_id",
                "label_name",
                "board_id",
                "polarity",
                "kind",
                "text",
                "ordinal",
                "content_hash",
                "embedding_model",
                "_distance",
            ];
            if query.include_vector {
                columns.push(VECTOR_COLUMN);
            }
            let stream = table
                .query()
                .nearest_to(query.vector.clone())
                .map_err(map_lancedb_error)?
                .column(VECTOR_COLUMN)
                .only_if_expr(filter)
                .limit(query.limit)
                .select(Select::columns(&columns))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            let batches = stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_lancedb_error)?;
            batches_to_label_atom_vector_hits(&batches, query.include_vector)
        })
    }
}

async fn ensure_table(
    connection: &lancedb::Connection,
    table_name: &str,
    dimensions: usize,
) -> Result<Table, VectorError> {
    match connection.open_table(table_name).execute().await {
        Ok(table) => {
            validate_vector_schema(&table, dimensions).await?;
            Ok(table)
        }
        Err(lancedb::Error::TableNotFound { .. }) => {
            let schema = vector_schema(dimensions);
            let table = connection
                .create_empty_table(table_name, schema)
                .mode(CreateTableMode::exist_ok(|request| request))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            validate_vector_schema(&table, dimensions).await?;
            Ok(table)
        }
        Err(err) => Err(map_lancedb_error(err)),
    }
}

async fn open_existing_table(
    connection: &lancedb::Connection,
    table_name: &str,
    dimensions: usize,
) -> Result<Table, VectorError> {
    let table = connection
        .open_table(table_name)
        .execute()
        .await
        .map_err(map_lancedb_error)?;
    validate_vector_schema(&table, dimensions).await?;
    Ok(table)
}

async fn ensure_label_atom_table(
    connection: &lancedb::Connection,
    table_name: &str,
    dimensions: usize,
) -> Result<Table, VectorError> {
    match connection.open_table(table_name).execute().await {
        Ok(table) => {
            validate_vector_schema(&table, dimensions).await?;
            Ok(table)
        }
        Err(lancedb::Error::TableNotFound { .. }) => {
            let schema = label_atom_schema(dimensions);
            let table = connection
                .create_empty_table(table_name, schema)
                .mode(CreateTableMode::exist_ok(|request| request))
                .execute()
                .await
                .map_err(map_lancedb_error)?;
            validate_vector_schema(&table, dimensions).await?;
            Ok(table)
        }
        Err(err) => Err(map_lancedb_error(err)),
    }
}

async fn validate_vector_schema(table: &Table, expected: usize) -> Result<(), VectorError> {
    let schema = table.schema().await.map_err(map_lancedb_error)?;
    let field = schema
        .field_with_name(VECTOR_COLUMN)
        .map_err(|err| VectorError::Store(err.to_string()))?;
    match field.data_type() {
        DataType::FixedSizeList(_, actual) if *actual as usize == expected => Ok(()),
        DataType::FixedSizeList(_, actual) => Err(VectorError::DimensionMismatch {
            expected,
            actual: *actual as usize,
        }),
        data_type => Err(VectorError::Store(format!(
            "vector column has non-vector type {data_type:?}"
        ))),
    }
}

fn vector_schema(dimensions: usize) -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("chunk_key", DataType::Utf8, false),
        Field::new("entity_uri", DataType::Utf8, false),
        Field::new("chunk_uri", DataType::Utf8, false),
        Field::new("kind", DataType::Utf8, false),
        Field::new("project_id", DataType::Utf8, true),
        Field::new("board_id", DataType::Utf8, true),
        Field::new("task_id", DataType::Utf8, true),
        Field::new("source_table", DataType::Utf8, false),
        Field::new("source_id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("summary", DataType::Utf8, true),
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new("content_hash", DataType::Utf8, true),
        Field::new("created_at", DataType::Int64, false),
        Field::new("updated_at", DataType::Int64, false),
        Field::new("source_event_id", DataType::Int64, true),
        Field::new("metadata_json", DataType::Utf8, false),
        Field::new("ordinal", DataType::Int64, false),
        Field::new(
            VECTOR_COLUMN,
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimensions as i32,
            ),
            false,
        ),
    ]))
}

fn label_atom_schema(dimensions: usize) -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("atom_key", DataType::Utf8, false),
        Field::new("atom_id", DataType::Utf8, false),
        Field::new("label_id", DataType::Utf8, false),
        Field::new("label_name", DataType::Utf8, false),
        Field::new("board_id", DataType::Utf8, false),
        Field::new("polarity", DataType::Utf8, false),
        Field::new("kind", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("ordinal", DataType::Int64, false),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new("created_at", DataType::Int64, false),
        Field::new("updated_at", DataType::Int64, false),
        Field::new(
            VECTOR_COLUMN,
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimensions as i32,
            ),
            false,
        ),
    ]))
}

fn chunks_to_batch(
    chunks: &[EmbeddingChunk],
    embeddings: &[Vec<f32>],
    dimensions: usize,
) -> Result<RecordBatch, VectorError> {
    let schema = vector_schema(dimensions);
    let chunk_keys: Vec<_> = chunks.iter().map(EmbeddingChunk::chunk_key).collect();
    let entity_uris: Vec<_> = chunks
        .iter()
        .map(|chunk| chunk.chunk.entity_uri.as_str().to_owned())
        .collect();
    let chunk_uris: Vec<_> = chunks
        .iter()
        .map(|chunk| chunk.chunk.uri.as_str().to_owned())
        .collect();
    let content_hashes: Vec<_> = chunks
        .iter()
        .map(|chunk| chunk.chunk.content_hash.clone())
        .collect();
    let ordinals: Vec<_> = chunks.iter().map(|chunk| chunk.chunk.ordinal).collect();
    let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        embeddings
            .iter()
            .map(|embedding| Some(embedding.iter().copied().map(Some))),
        dimensions as i32,
    );

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(chunk_keys)),
            Arc::new(StringArray::from(entity_uris)),
            Arc::new(StringArray::from(chunk_uris)),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.kind.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.project_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.board_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.task_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.source_table.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.source_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.text.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.summary.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.embedding_model.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(content_hashes)),
            Arc::new(Int64Array::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.created_at)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.updated_at)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.source_event_id)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|chunk| chunk.metadata_json.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(ordinals)),
            Arc::new(vectors),
        ],
    )
    .map_err(|err| VectorError::Store(err.to_string()))
}

fn label_atoms_to_batch(
    atoms: &[LabelAtomVector],
    embeddings: &[Vec<f32>],
    dimensions: usize,
) -> Result<RecordBatch, VectorError> {
    let schema = label_atom_schema(dimensions);
    let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        embeddings
            .iter()
            .map(|embedding| Some(embedding.iter().copied().map(Some))),
        dimensions as i32,
    );

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(LabelAtomVector::atom_key)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.atom_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.label_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.label_name.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.board_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.polarity.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.kind.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.text.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                atoms.iter().map(|atom| atom.ordinal).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.content_hash.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                atoms
                    .iter()
                    .map(|atom| atom.embedding_model.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                atoms.iter().map(|atom| atom.created_at).collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                atoms.iter().map(|atom| atom.updated_at).collect::<Vec<_>>(),
            )),
            Arc::new(vectors),
        ],
    )
    .map_err(|err| VectorError::Store(err.to_string()))
}

fn batches_to_hits(batches: &[RecordBatch]) -> Result<Vec<VectorHit>, VectorError> {
    let mut hits = Vec::new();
    for batch in batches {
        let chunk_uri = string_column(batch, "chunk_uri")?;
        let entity_uri = string_column(batch, "entity_uri")?;
        let ordinal = int64_column(batch, "ordinal")?;
        let content_hash = string_column(batch, "content_hash")?;
        let text = string_column(batch, "text")?;
        let summary = string_column(batch, "summary")?;
        let distance = float32_column(batch, "_distance")?;

        for row in 0..batch.num_rows() {
            let chunk_uri =
                kanban_entity::EntityUri::new(required_string(chunk_uri, row, "chunk_uri")?)
                    .map_err(|err| VectorError::Store(err.to_string()))?;
            let entity_uri =
                kanban_entity::EntityUri::new(required_string(entity_uri, row, "entity_uri")?)
                    .map_err(|err| VectorError::Store(err.to_string()))?;
            hits.push(VectorHit {
                chunk: kanban_entity::ChunkRef {
                    uri: chunk_uri,
                    entity_uri,
                    ordinal: required_i64(ordinal, row, "ordinal")?,
                    content_hash: optional_string(content_hash, row),
                },
                score: required_f32(distance, row, "_distance")?,
                text: Some(required_string(text, row, "text")?.to_owned()),
                summary: optional_string(summary, row),
            });
        }
    }
    Ok(hits)
}

fn batches_to_label_atom_hits(batches: &[RecordBatch]) -> Result<Vec<LabelAtomHit>, VectorError> {
    let mut hits = Vec::new();
    for batch in batches {
        let atom_id = string_column(batch, "atom_id")?;
        let label_id = string_column(batch, "label_id")?;
        let label_name = string_column(batch, "label_name")?;
        let board_id = string_column(batch, "board_id")?;
        let polarity = string_column(batch, "polarity")?;
        let kind = string_column(batch, "kind")?;
        let text = string_column(batch, "text")?;
        let ordinal = int64_column(batch, "ordinal")?;
        let content_hash = string_column(batch, "content_hash")?;
        let embedding_model = string_column(batch, "embedding_model")?;
        let distance = float32_column(batch, "_distance")?;

        for row in 0..batch.num_rows() {
            hits.push(LabelAtomHit {
                atom_id: required_string(atom_id, row, "atom_id")?.to_owned(),
                label_id: required_string(label_id, row, "label_id")?.to_owned(),
                label_name: required_string(label_name, row, "label_name")?.to_owned(),
                board_id: required_string(board_id, row, "board_id")?.to_owned(),
                polarity: required_string(polarity, row, "polarity")?.to_owned(),
                kind: required_string(kind, row, "kind")?.to_owned(),
                text: required_string(text, row, "text")?.to_owned(),
                ordinal: required_i64(ordinal, row, "ordinal")?,
                content_hash: required_string(content_hash, row, "content_hash")?.to_owned(),
                embedding_model: required_string(embedding_model, row, "embedding_model")?
                    .to_owned(),
                distance: required_f32(distance, row, "_distance")?,
            });
        }
    }
    Ok(hits)
}

fn batches_to_label_atom_vector_hits(
    batches: &[RecordBatch],
    include_vector: bool,
) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
    let mut hits = Vec::new();
    for batch in batches {
        let atom_id = string_column(batch, "atom_id")?;
        let label_id = string_column(batch, "label_id")?;
        let label_name = string_column(batch, "label_name")?;
        let board_id = string_column(batch, "board_id")?;
        let polarity = string_column(batch, "polarity")?;
        let kind = string_column(batch, "kind")?;
        let text = string_column(batch, "text")?;
        let ordinal = int64_column(batch, "ordinal")?;
        let content_hash = string_column(batch, "content_hash")?;
        let embedding_model = string_column(batch, "embedding_model")?;
        let distance = float32_column(batch, "_distance")?;
        let vectors = include_vector
            .then(|| fixed_size_list_column(batch, VECTOR_COLUMN))
            .transpose()?;

        for row in 0..batch.num_rows() {
            hits.push(LabelAtomVectorHit {
                hit: LabelAtomHit {
                    atom_id: required_string(atom_id, row, "atom_id")?.to_owned(),
                    label_id: required_string(label_id, row, "label_id")?.to_owned(),
                    label_name: required_string(label_name, row, "label_name")?.to_owned(),
                    board_id: required_string(board_id, row, "board_id")?.to_owned(),
                    polarity: required_string(polarity, row, "polarity")?.to_owned(),
                    kind: required_string(kind, row, "kind")?.to_owned(),
                    text: required_string(text, row, "text")?.to_owned(),
                    ordinal: required_i64(ordinal, row, "ordinal")?,
                    content_hash: required_string(content_hash, row, "content_hash")?.to_owned(),
                    embedding_model: required_string(
                        embedding_model,
                        row,
                        "embedding_model",
                    )?
                    .to_owned(),
                    distance: required_f32(distance, row, "_distance")?,
                },
                vector: vectors
                    .as_ref()
                    .map(|vectors| fixed_size_list_value(vectors, row))
                    .transpose()?,
            });
        }
    }
    Ok(hits)
}

fn chunk_batches_to_projection_rows(
    batches: &[RecordBatch],
) -> Result<Vec<ProjectionContentRow>, VectorError> {
    let mut rows = Vec::new();
    for batch in batches {
        let chunk_key = string_column(batch, "chunk_key")?;
        let entity_uri = string_column(batch, "entity_uri")?;
        let chunk_uri = string_column(batch, "chunk_uri")?;
        let kind = string_column(batch, "kind")?;
        let project_id = string_column(batch, "project_id")?;
        let board_id = string_column(batch, "board_id")?;
        let task_id = string_column(batch, "task_id")?;
        let source_table = string_column(batch, "source_table")?;
        let source_id = string_column(batch, "source_id")?;
        let text = string_column(batch, "text")?;
        let summary = string_column(batch, "summary")?;
        let embedding_model = string_column(batch, "embedding_model")?;
        let content_hash = string_column(batch, "content_hash")?;
        let created_at = int64_column(batch, "created_at")?;
        let updated_at = int64_column(batch, "updated_at")?;
        let source_event_id = int64_column(batch, "source_event_id")?;
        let metadata_json = string_column(batch, "metadata_json")?;
        let ordinal = int64_column(batch, "ordinal")?;
        let vectors = fixed_size_list_column(batch, VECTOR_COLUMN)?;
        for row in 0..batch.num_rows() {
            let chunk_key_value = required_string(&chunk_key, row, "chunk_key")?;
            rows.push(ProjectionContentRow {
                key: chunk_key_value.to_owned(),
                content_json: serde_json::json!({
                    "chunk_key": chunk_key_value,
                    "entity_uri": required_string(&entity_uri, row, "entity_uri")?,
                    "chunk_uri": required_string(&chunk_uri, row, "chunk_uri")?,
                    "kind": required_string(&kind, row, "kind")?,
                    "project_id": optional_string(project_id, row),
                    "board_id": optional_string(board_id, row),
                    "task_id": optional_string(task_id, row),
                    "source_table": required_string(&source_table, row, "source_table")?,
                    "source_id": required_string(&source_id, row, "source_id")?,
                    "text": required_string(&text, row, "text")?,
                    "summary": optional_string(summary, row),
                    "embedding_model": required_string(&embedding_model, row, "embedding_model")?,
                    "content_hash": optional_string(content_hash, row),
                    "created_at": required_i64(&created_at, row, "created_at")?,
                    "updated_at": required_i64(&updated_at, row, "updated_at")?,
                    "source_event_id": optional_i64(source_event_id, row),
                    "metadata_json": required_string(&metadata_json, row, "metadata_json")?,
                    "ordinal": required_i64(&ordinal, row, "ordinal")?,
                })
                .to_string(),
                vector_bits: Some(
                    fixed_size_list_value(vectors, row)?
                        .into_iter()
                        .map(f32::to_bits)
                        .collect(),
                ),
            });
        }
    }
    normalize_projection_rows(rows)
}

fn label_atom_batches_to_projection_rows(
    batches: &[RecordBatch],
) -> Result<Vec<ProjectionContentRow>, VectorError> {
    let mut rows = Vec::new();
    for batch in batches {
        let atom_key = string_column(batch, "atom_key")?;
        let atom_id = string_column(batch, "atom_id")?;
        let label_id = string_column(batch, "label_id")?;
        let label_name = string_column(batch, "label_name")?;
        let board_id = string_column(batch, "board_id")?;
        let polarity = string_column(batch, "polarity")?;
        let kind = string_column(batch, "kind")?;
        let text = string_column(batch, "text")?;
        let ordinal = int64_column(batch, "ordinal")?;
        let content_hash = string_column(batch, "content_hash")?;
        let embedding_model = string_column(batch, "embedding_model")?;
        let created_at = int64_column(batch, "created_at")?;
        let updated_at = int64_column(batch, "updated_at")?;
        let vectors = fixed_size_list_column(batch, VECTOR_COLUMN)?;
        for row in 0..batch.num_rows() {
            let atom_key_value = required_string(&atom_key, row, "atom_key")?;
            rows.push(ProjectionContentRow {
                key: atom_key_value.to_owned(),
                content_json: serde_json::json!({
                    "atom_key": atom_key_value,
                    "atom_id": required_string(&atom_id, row, "atom_id")?,
                    "label_id": required_string(&label_id, row, "label_id")?,
                    "label_name": required_string(&label_name, row, "label_name")?,
                    "board_id": required_string(&board_id, row, "board_id")?,
                    "polarity": required_string(&polarity, row, "polarity")?,
                    "kind": required_string(&kind, row, "kind")?,
                    "text": required_string(&text, row, "text")?,
                    "ordinal": required_i64(&ordinal, row, "ordinal")?,
                    "content_hash": required_string(&content_hash, row, "content_hash")?,
                    "embedding_model": required_string(&embedding_model, row, "embedding_model")?,
                    "created_at": required_i64(&created_at, row, "created_at")?,
                    "updated_at": required_i64(&updated_at, row, "updated_at")?,
                })
                .to_string(),
                vector_bits: Some(
                    fixed_size_list_value(vectors, row)?
                        .into_iter()
                        .map(f32::to_bits)
                        .collect(),
                ),
            });
        }
    }
    normalize_projection_rows(rows)
}

fn normalize_projection_rows(
    mut rows: Vec<ProjectionContentRow>,
) -> Result<Vec<ProjectionContentRow>, VectorError> {
    rows.sort();
    if rows.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(VectorError::Store(
            "projection table contains duplicate stable row keys".to_owned(),
        ));
    }
    Ok(rows)
}

fn string_column<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray, VectorError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| VectorError::Store(format!("missing string column {name}")))
}

fn int64_column<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Int64Array, VectorError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<Int64Array>())
        .ok_or_else(|| VectorError::Store(format!("missing int64 column {name}")))
}

fn required_string<'a>(
    array: &'a StringArray,
    row: usize,
    name: &str,
) -> Result<&'a str, VectorError> {
    if array.is_null(row) {
        return Err(VectorError::Store(format!(
            "projection required string column {name} contains a null row"
        )));
    }
    Ok(array.value(row))
}

fn required_i64(array: &Int64Array, row: usize, name: &str) -> Result<i64, VectorError> {
    if array.is_null(row) {
        return Err(VectorError::Store(format!(
            "projection required int64 column {name} contains a null row"
        )));
    }
    Ok(array.value(row))
}

fn required_f32(array: &Float32Array, row: usize, name: &str) -> Result<f32, VectorError> {
    if array.is_null(row) {
        return Err(VectorError::Store(format!(
            "projection required float32 column {name} contains a null row"
        )));
    }
    Ok(array.value(row))
}

fn float32_column<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Float32Array, VectorError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<Float32Array>())
        .ok_or_else(|| VectorError::Store(format!("missing float32 column {name}")))
}

fn fixed_size_list_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a FixedSizeListArray, VectorError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<FixedSizeListArray>())
        .ok_or_else(|| VectorError::Store(format!("missing fixed-size list column {name}")))
}

fn fixed_size_list_value(array: &FixedSizeListArray, row: usize) -> Result<Vec<f32>, VectorError> {
    if array.is_null(row) {
        return Err(VectorError::Store(
            "projection vector row contains a null vector".to_owned(),
        ));
    }
    let value = array.value(row);
    let values = value
        .as_any()
        .downcast_ref::<Float32Array>()
        .ok_or_else(|| VectorError::Store("label atom vector values are not float32".to_owned()))?;
    if values.null_count() != 0 {
        return Err(VectorError::Store(
            "projection vector row contains a null coordinate".to_owned(),
        ));
    }
    let coordinates = (0..values.len())
        .map(|index| values.value(index))
        .collect::<Vec<_>>();
    if coordinates.iter().any(|coordinate| !coordinate.is_finite()) {
        return Err(VectorError::Store(
            "projection vector row contains a non-finite coordinate".to_owned(),
        ));
    }
    Ok(coordinates)
}

fn optional_string(array: &StringArray, row: usize) -> Option<String> {
    (!array.is_null(row)).then(|| array.value(row).to_owned())
}

fn optional_i64(array: &Int64Array, row: usize) -> Option<i64> {
    (!array.is_null(row)).then(|| array.value(row))
}

fn path_string(config: &LanceDbConfig) -> Result<String, VectorError> {
    config
        .path
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| VectorError::Store("LanceDB path is not valid UTF-8".to_owned()))
}

fn map_lancedb_error(err: lancedb::Error) -> VectorError {
    VectorError::Store(err.to_string())
}

fn embed_deduplicated<'a>(
    provider: &dyn EmbeddingProvider,
    texts: impl IntoIterator<Item = &'a str>,
    policy: &crate::EmbeddingExecutionPolicy,
) -> Result<Vec<Vec<f32>>, VectorError> {
    let mut unique_by_hash = HashMap::<String, usize>::new();
    let mut unique_texts = Vec::<String>::new();
    let mut source_indexes = Vec::<usize>::new();

    for text in texts {
        let normalized = normalize_semantic_text(text);
        let content_hash = semantic_content_hash(&normalized);
        let unique_index = match unique_by_hash.get(&content_hash) {
            Some(index) => *index,
            None => {
                let index = unique_texts.len();
                unique_texts.push(normalized);
                unique_by_hash.insert(content_hash, index);
                index
            }
        };
        source_indexes.push(unique_index);
    }

    let mut unique_embeddings = Vec::with_capacity(unique_texts.len());
    for (batch_index, batch) in unique_texts.chunks(policy.batch_size).enumerate() {
        if batch_index > 0 && !policy.min_batch_interval.is_zero() {
            std::thread::sleep(policy.min_batch_interval);
        }
        let embeddings = embed_batch_with_retry(provider, batch, policy)?;
        if embeddings.len() != batch.len() {
            return Err(VectorError::Store(format!(
                "embedding provider batch cardinality mismatch: expected {}, got {}",
                batch.len(),
                embeddings.len()
            )));
        }
        for embedding in &embeddings {
            ensure_dimensions(embedding, provider.dimensions())?;
        }
        unique_embeddings.extend(embeddings);
    }

    source_indexes
        .into_iter()
        .map(|index| {
            unique_embeddings
                .get(index)
                .cloned()
                .ok_or_else(|| VectorError::Store("embedding batch index was missing".to_owned()))
        })
        .collect()
}

fn embed_batch_with_retry(
    provider: &dyn EmbeddingProvider,
    texts: &[String],
    policy: &crate::EmbeddingExecutionPolicy,
) -> Result<Vec<Vec<f32>>, VectorError> {
    let mut retry_backoff = policy.initial_retry_backoff;
    for attempt in 0..=policy.max_retries {
        match provider.embed_batch(texts) {
            Ok(embeddings) => return Ok(embeddings),
            Err(error) if error.is_retryable() && attempt < policy.max_retries => {
                let delay = retry_backoff.max(policy.min_batch_interval);
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                retry_backoff = retry_backoff
                    .saturating_mul(2)
                    .min(policy.max_retry_backoff);
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("embedding retry loop always returns")
}

#[cfg(test)]
mod tests {
    use arrow_array::{ArrayRef, Float32Array, RecordBatch, new_null_array};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    use crate::{EmbeddingExecutionPolicy, LanceDbConfig, LanceDbStore};
    use kanban_vector::{
        ChunkBuilder, ChunkVectorStore, EmbeddingProvider, LabelAtomQuery, LabelAtomVector,
        LabelAtomVectorQuery, LabelAtomVectorStore, TaskChunkSource, VectorError, VectorQuery,
        VectorStoreBackend,
    };

    struct StaticProvider;

    impl EmbeddingProvider for StaticProvider {
        fn embedding_model(&self) -> &str {
            "static-test"
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError> {
            Ok(match text {
                text if text.contains("alpha") => vec![1.0, 0.0, 0.0],
                text if text.contains("beta") => vec![0.0, 1.0, 0.0],
                _ => vec![0.0, 0.0, 1.0],
            })
        }
    }

    struct BadProvider;

    impl EmbeddingProvider for BadProvider {
        fn embedding_model(&self) -> &str {
            "bad-test"
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn embed(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
            Ok(vec![1.0, 0.0])
        }
    }

    struct OtherModelProvider;

    impl EmbeddingProvider for OtherModelProvider {
        fn embedding_model(&self) -> &str {
            "other-test"
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn embed(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
            Ok(vec![1.0, 0.0, 0.0])
        }
    }

    struct BatchCountingProvider {
        embed_calls: Arc<AtomicUsize>,
        batch_calls: Arc<AtomicUsize>,
        batch_items: Arc<AtomicUsize>,
    }

    impl EmbeddingProvider for BatchCountingProvider {
        fn embedding_model(&self) -> &str {
            "batch-test"
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn embed(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
            self.embed_calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![1.0, 0.0, 0.0])
        }

        fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, VectorError> {
            self.batch_calls.fetch_add(1, Ordering::SeqCst);
            self.batch_items.fetch_add(texts.len(), Ordering::SeqCst);
            Ok(vec![vec![1.0, 0.0, 0.0]; texts.len()])
        }
    }

    struct RetryOnceProvider {
        calls: Arc<AtomicUsize>,
    }

    impl EmbeddingProvider for RetryOnceProvider {
        fn embedding_model(&self) -> &str {
            "retry-test"
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn embed(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
            unreachable!("LanceDB writes must use the batch provider seam")
        }

        fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, VectorError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                return Err(VectorError::Provider {
                    message: "temporary provider outage".to_owned(),
                    retryable: true,
                });
            }
            Ok(vec![vec![1.0, 0.0, 0.0]; texts.len()])
        }
    }

    fn test_execution_policy(batch_size: usize) -> EmbeddingExecutionPolicy {
        EmbeddingExecutionPolicy {
            batch_size,
            min_batch_interval: Duration::ZERO,
            max_retries: 2,
            initial_retry_backoff: Duration::ZERO,
            max_retry_backoff: Duration::ZERO,
        }
    }

    #[test]
    fn degraded_lancedb_store_reports_unavailable_without_provider() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = LanceDbStore::connect(LanceDbConfig::degraded(tempdir.path())).unwrap();

        assert!(!store.status().enabled);
        assert!(matches!(
            store.upsert(&[]),
            Err(VectorError::MissingEmbeddingProvider)
        ));
        assert!(matches!(
            store.delete_entities(&["kb://task/t_1".to_owned()]),
            Err(VectorError::MissingEmbeddingProvider)
        ));
        assert!(matches!(
            store.delete_board("b_1"),
            Err(VectorError::MissingEmbeddingProvider)
        ));
        assert!(matches!(
            store.query_label_atoms_by_vector(&LabelAtomVectorQuery {
                vector: vec![1.0, 0.0, 0.0],
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("static-test".to_owned()),
                polarity: None,
                include_vector: false,
            }),
            Err(VectorError::MissingEmbeddingProvider)
        ));
    }

    #[test]
    fn lancedb_store_upserts_chunks_and_queries_hits() {
        let tempdir = tempfile::tempdir().unwrap();
        let provider = Arc::new(StaticProvider);
        let store = LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), provider)).unwrap();
        let builder = ChunkBuilder::new("static-test");
        let chunks = vec![
            build_chunk(&builder, "t_alpha", "alpha work"),
            build_chunk(&builder, "t_beta", "beta work"),
        ];

        store.upsert(&chunks).unwrap();
        store
            .upsert(&[build_chunk(&builder, "t_alpha", "alpha updated")])
            .unwrap();
        let hits = store
            .query(&VectorQuery {
                text: "alpha".to_owned(),
                limit: 1,
                board_id: "b_1".to_owned(),
            })
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk.uri.as_str(), "kb://chunk/task/t_alpha/0");
        assert_eq!(hits[0].text.as_deref(), Some("alpha updated"));
    }

    #[test]
    fn lancedb_store_deletes_chunks_by_entity_uri() {
        let tempdir = tempfile::tempdir().unwrap();
        let provider = Arc::new(StaticProvider);
        let store = LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), provider)).unwrap();
        let builder = ChunkBuilder::new("static-test");
        let chunks = vec![
            build_chunk(&builder, "t_alpha", "alpha work"),
            build_chunk(&builder, "t_beta", "beta work"),
        ];

        store.upsert(&chunks).unwrap();
        store
            .delete_entities(&["kb://task/t_alpha".to_owned()])
            .unwrap();
        let hits = store
            .query(&VectorQuery {
                text: "alpha".to_owned(),
                limit: 10,
                board_id: "b_1".to_owned(),
            })
            .unwrap();

        assert!(
            hits.iter()
                .all(|hit| hit.chunk.entity_uri.as_str() != "kb://task/t_alpha"),
            "{hits:?}"
        );
    }

    #[test]
    fn lancedb_store_deletes_chunks_by_board_id() {
        let tempdir = tempfile::tempdir().unwrap();
        let provider = Arc::new(StaticProvider);
        let store = LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), provider)).unwrap();
        let builder = ChunkBuilder::new("static-test");
        let mut first_board = build_chunk(&builder, "t_alpha", "alpha work");
        first_board.board_id = Some("b_first".to_owned());
        let mut second_board = build_chunk(&builder, "t_beta", "beta work");
        second_board.board_id = Some("b_second".to_owned());

        store.upsert(&[first_board, second_board]).unwrap();
        store.delete_board("b_first").unwrap();
        let hits = store
            .query(&VectorQuery {
                text: "alpha".to_owned(),
                limit: 10,
                board_id: "b_second".to_owned(),
            })
            .unwrap();

        assert!(
            hits.iter()
                .all(|hit| hit.chunk.entity_uri.as_str() != "kb://task/t_alpha"),
            "{hits:?}"
        );
        assert!(
            hits.iter()
                .any(|hit| hit.chunk.entity_uri.as_str() == "kb://task/t_beta"),
            "{hits:?}"
        );
    }

    #[test]
    fn lancedb_store_queries_chunks_with_mandatory_board_scope() {
        let tempdir = tempfile::tempdir().unwrap();
        let provider = Arc::new(StaticProvider);
        let store = LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), provider)).unwrap();
        let builder = ChunkBuilder::new("static-test");
        let mut first_board = build_chunk(&builder, "t_alpha", "shared work");
        first_board.board_id = Some("b_first".to_owned());
        let mut second_board = build_chunk(&builder, "t_beta", "shared work");
        second_board.board_id = Some("b_second".to_owned());

        store.upsert(&[first_board, second_board]).unwrap();
        let hits = store
            .query(&VectorQuery {
                text: "shared".to_owned(),
                limit: 10,
                board_id: "b_first".to_owned(),
            })
            .unwrap();

        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].chunk.entity_uri.as_str(), "kb://task/t_alpha");
    }

    #[test]
    fn lancedb_store_batches_and_deduplicates_identical_chunk_embeddings() {
        let tempdir = tempfile::tempdir().unwrap();
        let embed_calls = Arc::new(AtomicUsize::new(0));
        let batch_calls = Arc::new(AtomicUsize::new(0));
        let batch_items = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(BatchCountingProvider {
            embed_calls: embed_calls.clone(),
            batch_calls: batch_calls.clone(),
            batch_items: batch_items.clone(),
        });
        let store = LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), provider)).unwrap();
        let builder = ChunkBuilder::new("batch-test");

        store
            .upsert(&[
                build_chunk(&builder, "t_alpha", "same semantic text"),
                build_chunk(&builder, "t_beta", "same semantic text"),
            ])
            .unwrap();

        assert_eq!(embed_calls.load(Ordering::SeqCst), 0);
        assert_eq!(batch_calls.load(Ordering::SeqCst), 1);
        assert_eq!(batch_items.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn lancedb_store_limits_embedding_provider_batch_size() {
        let tempdir = tempfile::tempdir().unwrap();
        let embed_calls = Arc::new(AtomicUsize::new(0));
        let batch_calls = Arc::new(AtomicUsize::new(0));
        let batch_items = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(BatchCountingProvider {
            embed_calls,
            batch_calls: batch_calls.clone(),
            batch_items: batch_items.clone(),
        });
        let config = LanceDbConfig::new(tempdir.path(), provider)
            .with_execution_policy(test_execution_policy(1));
        let store = LanceDbStore::connect(config).unwrap();
        let builder = ChunkBuilder::new("batch-test");

        store
            .upsert(&[
                build_chunk(&builder, "t_alpha", "first unique text"),
                build_chunk(&builder, "t_beta", "second unique text"),
            ])
            .unwrap();

        assert_eq!(batch_calls.load(Ordering::SeqCst), 2);
        assert_eq!(batch_items.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn lancedb_store_retries_only_retryable_provider_failures() {
        let tempdir = tempfile::tempdir().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(RetryOnceProvider {
            calls: calls.clone(),
        });
        let config = LanceDbConfig::new(tempdir.path(), provider)
            .with_execution_policy(test_execution_policy(8));
        let store = LanceDbStore::connect(config).unwrap();
        let builder = ChunkBuilder::new("retry-test");

        store
            .upsert(&[build_chunk(&builder, "t_alpha", "retry text")])
            .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn lancedb_store_scopes_delete_and_query_by_embedding_model() {
        let tempdir = tempfile::tempdir().unwrap();
        let static_store =
            LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), Arc::new(StaticProvider)))
                .unwrap();
        let other_store = LanceDbStore::connect(LanceDbConfig::new(
            tempdir.path(),
            Arc::new(OtherModelProvider),
        ))
        .unwrap();
        let static_builder = ChunkBuilder::new("static-test");
        let other_builder = ChunkBuilder::new("other-test");

        other_store
            .upsert(&[build_chunk(&other_builder, "t_other", "shared text")])
            .unwrap();
        assert!(
            static_store
                .query(&VectorQuery {
                    text: "shared".to_owned(),
                    limit: 10,
                    board_id: "b_1".to_owned(),
                })
                .unwrap()
                .is_empty()
        );

        static_store
            .upsert(&[build_chunk(&static_builder, "t_shared", "shared text")])
            .unwrap();
        static_store
            .delete_entities(&["kb://task/t_shared".to_owned()])
            .unwrap();
        let other_hits = other_store
            .query(&VectorQuery {
                text: "shared".to_owned(),
                limit: 10,
                board_id: "b_1".to_owned(),
            })
            .unwrap();
        assert_eq!(other_hits.len(), 1);
        assert_eq!(other_hits[0].chunk.entity_uri.as_str(), "kb://task/t_other");

        static_store
            .upsert(&[build_chunk(&static_builder, "t_static", "shared text")])
            .unwrap();
        static_store.delete_board("b_1").unwrap();
        assert!(
            static_store
                .query(&VectorQuery {
                    text: "shared".to_owned(),
                    limit: 10,
                    board_id: "b_1".to_owned(),
                })
                .unwrap()
                .is_empty()
        );
        let other_hits = other_store
            .query(&VectorQuery {
                text: "shared".to_owned(),
                limit: 10,
                board_id: "b_1".to_owned(),
            })
            .unwrap();
        assert_eq!(other_hits.len(), 1);
        assert_eq!(other_hits[0].chunk.entity_uri.as_str(), "kb://task/t_other");
    }

    #[test]
    fn lancedb_store_upserts_and_queries_label_atoms_in_separate_table() {
        let tempdir = tempfile::tempdir().unwrap();
        let static_store =
            LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), Arc::new(StaticProvider)))
                .unwrap();
        let other_store = LanceDbStore::connect(LanceDbConfig::new(
            tempdir.path(),
            Arc::new(OtherModelProvider),
        ))
        .unwrap();

        static_store
            .upsert_label_atoms(&[
                build_label_atom(
                    "la_alpha",
                    "l_alpha",
                    "b_1",
                    "positive",
                    "static-test",
                    "alpha atom",
                ),
                build_label_atom(
                    "la_beta",
                    "l_beta",
                    "b_2",
                    "negative",
                    "static-test",
                    "beta atom",
                ),
            ])
            .unwrap();
        other_store
            .upsert_label_atoms(&[build_label_atom(
                "la_other",
                "l_other",
                "b_1",
                "positive",
                "other-test",
                "alpha other",
            )])
            .unwrap();

        let hits = static_store
            .query_label_atoms(&LabelAtomQuery {
                text: "alpha".to_owned(),
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("static-test".to_owned()),
                polarity: Some("positive".to_owned()),
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].atom_id, "la_alpha");

        assert!(matches!(
            static_store.query_label_atoms(&LabelAtomQuery {
                text: "alpha".to_owned(),
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("other-test".to_owned()),
                polarity: None,
            }),
            Err(VectorError::EmbeddingModelMismatch {
                expected,
                actual
            }) if expected == "static-test" && actual == "other-test"
        ));
        assert!(
            static_store
                .query(&VectorQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: "b_1".to_owned(),
                })
                .unwrap()
                .is_empty(),
            "label atoms must not populate kb_chunks"
        );

        static_store
            .upsert(&[build_chunk(
                &ChunkBuilder::new("static-test"),
                "t_alpha",
                "alpha work",
            )])
            .unwrap();
        assert_eq!(
            static_store
                .query(&VectorQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: "b_1".to_owned(),
                })
                .unwrap()
                .len(),
            1
        );

        static_store.delete_label_atoms_for_board("b_1").unwrap();
        assert!(
            static_store
                .query_label_atoms(&LabelAtomQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: Some("b_1".to_owned()),
                    embedding_model: Some("static-test".to_owned()),
                    polarity: None,
                })
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            static_store
                .query(&VectorQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: "b_1".to_owned(),
                })
                .unwrap()
                .len(),
            1,
            "deleting label atoms must not delete kb_chunks rows"
        );
        assert_eq!(
            other_store
                .query_label_atoms(&LabelAtomQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: Some("b_1".to_owned()),
                    embedding_model: Some("other-test".to_owned()),
                    polarity: None,
                })
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn lancedb_store_queries_label_atoms_by_raw_vector_and_can_return_vectors() {
        let tempdir = tempfile::tempdir().unwrap();
        let static_store =
            LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), Arc::new(StaticProvider)))
                .unwrap();
        let other_store = LanceDbStore::connect(LanceDbConfig::new(
            tempdir.path(),
            Arc::new(OtherModelProvider),
        ))
        .unwrap();

        static_store
            .upsert_label_atoms(&[
                build_label_atom(
                    "la_alpha",
                    "l_alpha",
                    "b_1",
                    "positive",
                    "static-test",
                    "alpha atom",
                ),
                build_label_atom(
                    "la_beta",
                    "l_beta",
                    "b_1",
                    "negative",
                    "static-test",
                    "beta atom",
                ),
                build_label_atom(
                    "la_other_board",
                    "l_other_board",
                    "b_2",
                    "positive",
                    "static-test",
                    "alpha other board",
                ),
            ])
            .unwrap();
        other_store
            .upsert_label_atoms(&[build_label_atom(
                "la_other_model",
                "l_other_model",
                "b_1",
                "positive",
                "other-test",
                "alpha other model",
            )])
            .unwrap();

        let hits = static_store
            .query_label_atoms_by_vector(&LabelAtomVectorQuery {
                vector: vec![1.0, 0.0, 0.0],
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("static-test".to_owned()),
                polarity: Some("positive".to_owned()),
                include_vector: true,
            })
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].hit.atom_id, "la_alpha");
        assert_eq!(hits[0].vector.as_deref(), Some([1.0, 0.0, 0.0].as_slice()));

        let hits_without_vectors = static_store
            .query_label_atoms_by_vector(&LabelAtomVectorQuery {
                vector: vec![1.0, 0.0, 0.0],
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("static-test".to_owned()),
                polarity: Some("positive".to_owned()),
                include_vector: false,
            })
            .unwrap();

        assert_eq!(hits_without_vectors.len(), 1);
        assert_eq!(hits_without_vectors[0].hit.atom_id, "la_alpha");
        assert_eq!(hits_without_vectors[0].vector, None);
        assert!(
            static_store
                .query(&VectorQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: "b_1".to_owned(),
                })
                .unwrap()
                .is_empty(),
            "label atom vector query must not populate kb_chunks"
        );

        assert!(
            static_store
                .query_label_atoms(&LabelAtomQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: Some("b_1".to_owned()),
                    embedding_model: Some("static-test".to_owned()),
                    polarity: Some("positive".to_owned()),
                })
                .unwrap()
                .iter()
                .all(|hit| hit.atom_id == "la_alpha"),
            "text label atom query behavior should remain available"
        );

        assert!(matches!(
            static_store.query_label_atoms(&LabelAtomQuery {
                text: "alpha".to_owned(),
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("other-test".to_owned()),
                polarity: None,
            }),
            Err(VectorError::EmbeddingModelMismatch {
                expected,
                actual
            }) if expected == "static-test" && actual == "other-test"
        ));

        assert!(matches!(
            static_store.query_label_atoms_by_vector(&LabelAtomVectorQuery {
                vector: vec![1.0, 0.0, 0.0],
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("other-test".to_owned()),
                polarity: None,
                include_vector: false,
            }),
            Err(VectorError::EmbeddingModelMismatch {
                expected,
                actual
            }) if expected == "static-test" && actual == "other-test"
        ));

        assert!(matches!(
            static_store.query_label_atoms_by_vector(&LabelAtomVectorQuery {
                vector: vec![1.0, 0.0],
                limit: 10,
                board_id: Some("b_1".to_owned()),
                embedding_model: Some("static-test".to_owned()),
                polarity: None,
                include_vector: false,
            }),
            Err(VectorError::DimensionMismatch {
                expected: 3,
                actual: 2
            })
        ));
    }

    #[test]
    fn lancedb_store_rejects_embedding_dimension_mismatch() {
        let tempdir = tempfile::tempdir().unwrap();
        let store =
            LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), Arc::new(BadProvider)))
                .unwrap();
        let builder = ChunkBuilder::new("bad-test");
        let chunk = build_chunk(&builder, "t_bad", "bad");

        let err = store.upsert(&[chunk]).unwrap_err();
        assert!(matches!(
            err,
            VectorError::DimensionMismatch {
                expected: 3,
                actual: 2
            }
        ));
    }

    #[test]
    fn lancedb_store_rejects_embedding_model_mismatch() {
        let tempdir = tempfile::tempdir().unwrap();
        let store =
            LanceDbStore::connect(LanceDbConfig::new(tempdir.path(), Arc::new(StaticProvider)))
                .unwrap();
        let builder = ChunkBuilder::new("other-model");
        let chunk = build_chunk(&builder, "t_alpha", "alpha work");

        let err = store.upsert(&[chunk]).unwrap_err();
        assert!(matches!(
            err,
            VectorError::EmbeddingModelMismatch {
                expected,
                actual
            } if expected == "static-test" && actual == "other-model"
        ));
    }

    #[test]
    fn projection_vector_reader_rejects_outer_and_coordinate_nulls() {
        let coordinate_null = arrow_array::FixedSizeListArray::from_iter_primitive::<
            arrow_array::types::Float32Type,
            _,
            _,
        >(vec![Some(vec![Some(1.0_f32), None])], 2);
        assert!(matches!(
            super::fixed_size_list_value(&coordinate_null, 0),
            Err(VectorError::Store(message)) if message.contains("null")
        ));

        let outer_null = arrow_array::FixedSizeListArray::from_iter_primitive::<
            arrow_array::types::Float32Type,
            _,
            _,
        >(vec![None::<Vec<Option<f32>>>], 2);
        assert!(matches!(
            super::fixed_size_list_value(&outer_null, 0),
            Err(VectorError::Store(message)) if message.contains("null")
        ));
    }

    #[test]
    fn projection_vector_reader_rejects_non_finite_coordinates() {
        for coordinate in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let non_finite = arrow_array::FixedSizeListArray::from_iter_primitive::<
                arrow_array::types::Float32Type,
                _,
                _,
            >(vec![Some(vec![Some(coordinate)])], 1);
            assert!(matches!(
                super::fixed_size_list_value(&non_finite, 0),
                Err(VectorError::Store(message)) if message.contains("non-finite")
            ));
        }
    }

    #[test]
    fn chunk_projection_reader_rejects_null_required_cells() {
        let builder = ChunkBuilder::new("static-test");
        let chunk = build_chunk(&builder, "t_null", "null");
        let batch = super::chunks_to_batch(&[chunk], &[vec![1.0, 0.0, 0.0]], 3).unwrap();
        assert_eq!(
            super::chunk_batches_to_projection_rows(std::slice::from_ref(&batch))
                .unwrap()
                .len(),
            1
        );
        for name in [
            "chunk_key", "entity_uri", "chunk_uri", "kind", "source_table", "source_id",
            "text", "embedding_model", "created_at", "updated_at", "metadata_json", "ordinal",
        ] {
            let index = batch.schema().index_of(name).unwrap();
            let mut columns = batch.columns().to_vec();
            columns[index] = new_null_array(batch.schema().field(index).data_type(), 1);
            let mut fields = batch.schema().fields().to_vec();
            fields[index] = Arc::new(batch.schema().field(index).as_ref().clone().with_nullable(true));
            let invalid = RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap();
            let error = super::chunk_batches_to_projection_rows(&[invalid]).unwrap_err();
            assert!(matches!(error, VectorError::Store(message) if message.contains(name)));
        }
    }

    #[test]
    fn label_atom_projection_reader_rejects_null_required_cells() {
        let atom = LabelAtomVector {
            atom_id: "atom-1".to_owned(),
            label_id: "label-1".to_owned(),
            label_name: "Label".to_owned(),
            board_id: "board-1".to_owned(),
            polarity: "positive".to_owned(),
            kind: "task".to_owned(),
            text: "text".to_owned(),
            ordinal: 0,
            content_hash: "hash".to_owned(),
            embedding_model: "static-test".to_owned(),
            created_at: 1,
            updated_at: 2,
        };
        let batch = super::label_atoms_to_batch(&[atom], &[vec![1.0, 0.0, 0.0]], 3).unwrap();
        assert_eq!(
            super::label_atom_batches_to_projection_rows(std::slice::from_ref(&batch))
                .unwrap()
                .len(),
            1
        );
        for name in [
            "atom_key", "atom_id", "label_id", "label_name", "board_id", "polarity", "kind",
            "text", "ordinal", "content_hash", "embedding_model", "created_at", "updated_at",
        ] {
            let index = batch.schema().index_of(name).unwrap();
            let mut columns = batch.columns().to_vec();
            columns[index] = new_null_array(batch.schema().field(index).data_type(), 1);
            let mut fields = batch.schema().fields().to_vec();
            fields[index] = Arc::new(batch.schema().field(index).as_ref().clone().with_nullable(true));
            let invalid = RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap();
            let error = super::label_atom_batches_to_projection_rows(&[invalid]).unwrap_err();
            assert!(matches!(error, VectorError::Store(message) if message.contains(name)));
        }
    }

    #[test]
    fn query_reader_rejects_null_required_chunk_hit_cells() {
        let builder = ChunkBuilder::new("static-test");
        let chunk = build_chunk(&builder, "t_query_null", "query null");
        let base = super::chunks_to_batch(&[chunk], &[vec![1.0, 0.0, 0.0]], 3).unwrap();
        let batch = query_batch(
            &base,
            &[
                "chunk_uri",
                "entity_uri",
                "ordinal",
                "content_hash",
                "text",
                "summary",
            ],
        );
        assert_eq!(
            super::batches_to_hits(std::slice::from_ref(&batch))
                .unwrap()
                .len(),
            1
        );
        for name in ["chunk_uri", "entity_uri", "ordinal", "text", "_distance"] {
            let invalid = batch_with_null_cell(&batch, name);
            let error = super::batches_to_hits(&[invalid]).unwrap_err();
            assert!(matches!(error, VectorError::Store(message) if message.contains(name)));
        }
    }

    #[test]
    fn query_reader_rejects_null_required_label_hit_cells() {
        let batch = label_query_batch(false);
        assert_eq!(
            super::batches_to_label_atom_hits(std::slice::from_ref(&batch))
                .unwrap()
                .len(),
            1
        );
        for name in [
            "atom_id",
            "label_id",
            "label_name",
            "board_id",
            "polarity",
            "kind",
            "text",
            "ordinal",
            "content_hash",
            "embedding_model",
            "_distance",
        ] {
            let invalid = batch_with_null_cell(&batch, name);
            let error = super::batches_to_label_atom_hits(&[invalid]).unwrap_err();
            assert!(matches!(error, VectorError::Store(message) if message.contains(name)));
        }
    }

    #[test]
    fn query_reader_rejects_null_required_label_vector_hit_cells() {
        let batch = label_query_batch(true);
        assert_eq!(
            super::batches_to_label_atom_vector_hits(std::slice::from_ref(&batch), true)
                .unwrap()
                .len(),
            1
        );
        for name in [
            "atom_id",
            "label_id",
            "label_name",
            "board_id",
            "polarity",
            "kind",
            "text",
            "ordinal",
            "content_hash",
            "embedding_model",
            "_distance",
            "vector",
        ] {
            let invalid = batch_with_null_cell(&batch, name);
            let error = super::batches_to_label_atom_vector_hits(&[invalid], true).unwrap_err();
            assert!(matches!(error, VectorError::Store(message) if message.contains("null") || message.contains(name)));
        }
    }

    fn query_batch(base: &RecordBatch, names: &[&str]) -> RecordBatch {
        let mut fields = names
            .iter()
            .map(|name| Arc::new(base.schema().field_with_name(name).unwrap().clone()))
            .collect::<Vec<_>>();
        let mut columns = names
            .iter()
            .map(|name| base.column_by_name(name).unwrap().clone())
            .collect::<Vec<ArrayRef>>();
        fields.push(Arc::new(Field::new("_distance", DataType::Float32, false)));
        columns.push(Arc::new(Float32Array::from(vec![0.25])));
        RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap()
    }

    fn batch_with_null_cell(batch: &RecordBatch, name: &str) -> RecordBatch {
        let index = batch.schema().index_of(name).unwrap();
        let mut columns = batch.columns().to_vec();
        columns[index] = new_null_array(batch.schema().field(index).data_type(), batch.num_rows());
        let mut fields = batch.schema().fields().to_vec();
        fields[index] = Arc::new(
            batch
                .schema()
                .field(index)
                .as_ref()
                .clone()
                .with_nullable(true),
        );
        RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap()
    }

    fn label_query_batch(include_vector: bool) -> RecordBatch {
        let atom = LabelAtomVector {
            atom_id: "atom-query-1".to_owned(),
            label_id: "label-query-1".to_owned(),
            label_name: "Query Label".to_owned(),
            board_id: "board-query-1".to_owned(),
            polarity: "positive".to_owned(),
            kind: "task".to_owned(),
            text: "query text".to_owned(),
            ordinal: 0,
            content_hash: "query-hash".to_owned(),
            embedding_model: "static-test".to_owned(),
            created_at: 1,
            updated_at: 2,
        };
        let base = super::label_atoms_to_batch(&[atom], &[vec![1.0, 0.0, 0.0]], 3).unwrap();
        let mut names = vec![
            "atom_id",
            "label_id",
            "label_name",
            "board_id",
            "polarity",
            "kind",
            "text",
            "ordinal",
            "content_hash",
            "embedding_model",
        ];
        if include_vector {
            names.push("vector");
        }
        query_batch(&base, &names)
    }

    fn build_chunk(
        builder: &ChunkBuilder,
        task_id: &str,
        title: &str,
    ) -> kanban_vector::EmbeddingChunk {
        builder
            .build_task_chunks(&TaskChunkSource {
                task_uri: format!("kb://task/{task_id}"),
                project_id: Some("project-a".to_owned()),
                board_id: Some("b_1".to_owned()),
                task_id: task_id.to_owned(),
                title: title.to_owned(),
                description: None,
                comments: String::new(),
                run_text: String::new(),
                event_text: String::new(),
                source_event_id: None,
                created_at: 1,
                updated_at: 2,
            })
            .unwrap()
            .remove(0)
    }

    fn build_label_atom(
        atom_id: &str,
        label_id: &str,
        board_id: &str,
        polarity: &str,
        embedding_model: &str,
        text: &str,
    ) -> LabelAtomVector {
        LabelAtomVector {
            atom_id: atom_id.to_owned(),
            label_id: label_id.to_owned(),
            label_name: label_id.trim_start_matches("l_").to_owned(),
            board_id: board_id.to_owned(),
            polarity: polarity.to_owned(),
            kind: "description".to_owned(),
            text: text.to_owned(),
            ordinal: 0,
            content_hash: format!("hash-{atom_id}"),
            embedding_model: embedding_model.to_owned(),
            created_at: 1,
            updated_at: 2,
        }
    }
}
