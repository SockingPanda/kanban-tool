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

use crate::{
    EmbeddingChunk, EmbeddingProvider, LabelAtomHit, LabelAtomQuery, LabelAtomVector,
    LabelAtomVectorHit, LabelAtomVectorQuery, LanceDbConfig, VectorError, VectorHit, VectorQuery,
    VectorStore, VectorStoreStatus, ensure_dimensions,
};

const VECTOR_COLUMN: &str = "vector";

pub struct LanceDbStore {
    config: LanceDbConfig,
    provider: Option<Arc<dyn EmbeddingProvider + Send + Sync>>,
    runtime: Runtime,
}

impl LanceDbStore {
    pub fn connect(config: LanceDbConfig) -> Result<Self, VectorError> {
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
}

impl VectorStore for LanceDbStore {
    fn chunk_embedding_model(&self) -> &str {
        self.provider
            .as_ref()
            .map(|provider| provider.embedding_model())
            .unwrap_or(crate::DEFAULT_EMBEDDING_MODEL)
    }

    fn status(&self) -> VectorStoreStatus {
        match self.provider.as_ref() {
            Some(provider) => VectorStoreStatus {
                backend: "lancedb".to_owned(),
                enabled: true,
                message: format!(
                    "LanceDB vector store enabled for model {} ({} dimensions)",
                    provider.embedding_model(),
                    provider.dimensions()
                ),
            },
            None => VectorStoreStatus {
                backend: "lancedb".to_owned(),
                enabled: false,
                message:
                    "LanceDB configured without an embedding provider; vector retrieval degraded"
                        .to_owned(),
            },
        }
    }

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

        let dimensions = provider.dimensions();
        let mut embeddings = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let embedding = provider.embed(&chunk.text)?;
            ensure_dimensions(&embedding, dimensions)?;
            embeddings.push(embedding);
        }

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
            let stream = table
                .query()
                .nearest_to(embedding)
                .map_err(map_lancedb_error)?
                .column(VECTOR_COLUMN)
                .only_if_expr(col("embedding_model").eq(lit(provider.embedding_model())))
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

    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        let provider = self.provider()?;
        let embedding = provider.embed(text)?;
        ensure_dimensions(&embedding, provider.dimensions())?;
        Ok(embedding)
    }

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
        let mut embeddings = Vec::with_capacity(atoms.len());
        for atom in atoms {
            let embedding = provider.embed(&atom.text)?;
            ensure_dimensions(&embedding, dimensions)?;
            embeddings.push(embedding);
        }

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
            return Ok(Vec::new());
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
            return Ok(Vec::new());
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
            let chunk_uri = kanban_entity::EntityUri::new(chunk_uri.value(row).to_owned())
                .map_err(|err| VectorError::Store(err.to_string()))?;
            let entity_uri = kanban_entity::EntityUri::new(entity_uri.value(row).to_owned())
                .map_err(|err| VectorError::Store(err.to_string()))?;
            hits.push(VectorHit {
                chunk: kanban_entity::ChunkRef {
                    uri: chunk_uri,
                    entity_uri,
                    ordinal: ordinal.value(row),
                    content_hash: optional_string(content_hash, row),
                },
                score: distance.value(row),
                text: optional_string(text, row),
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
                atom_id: atom_id.value(row).to_owned(),
                label_id: label_id.value(row).to_owned(),
                label_name: label_name.value(row).to_owned(),
                board_id: board_id.value(row).to_owned(),
                polarity: polarity.value(row).to_owned(),
                kind: kind.value(row).to_owned(),
                text: text.value(row).to_owned(),
                ordinal: ordinal.value(row),
                content_hash: content_hash.value(row).to_owned(),
                embedding_model: embedding_model.value(row).to_owned(),
                score: distance.value(row),
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
                    atom_id: atom_id.value(row).to_owned(),
                    label_id: label_id.value(row).to_owned(),
                    label_name: label_name.value(row).to_owned(),
                    board_id: board_id.value(row).to_owned(),
                    polarity: polarity.value(row).to_owned(),
                    kind: kind.value(row).to_owned(),
                    text: text.value(row).to_owned(),
                    ordinal: ordinal.value(row),
                    content_hash: content_hash.value(row).to_owned(),
                    embedding_model: embedding_model.value(row).to_owned(),
                    score: distance.value(row),
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
    let value = array.value(row);
    let values = value
        .as_any()
        .downcast_ref::<Float32Array>()
        .ok_or_else(|| VectorError::Store("label atom vector values are not float32".to_owned()))?;
    Ok((0..values.len()).map(|index| values.value(index)).collect())
}

fn optional_string(array: &StringArray, row: usize) -> Option<String> {
    (!array.is_null(row)).then(|| array.value(row).to_owned())
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        ChunkBuilder, EmbeddingProvider, LabelAtomQuery, LabelAtomVector, LabelAtomVectorQuery,
        LanceDbConfig, LanceDbStore, TaskChunkSource, VectorError, VectorQuery, VectorStore,
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
                })
                .unwrap()
                .is_empty()
        );
        let other_hits = other_store
            .query(&VectorQuery {
                text: "shared".to_owned(),
                limit: 10,
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

        assert!(
            static_store
                .query_label_atoms(&LabelAtomQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
                    board_id: Some("b_1".to_owned()),
                    embedding_model: Some("other-test".to_owned()),
                    polarity: None,
                })
                .unwrap()
                .is_empty()
        );
        assert!(
            static_store
                .query(&VectorQuery {
                    text: "alpha".to_owned(),
                    limit: 10,
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

        assert!(
            static_store
                .query_label_atoms_by_vector(&LabelAtomVectorQuery {
                    vector: vec![1.0, 0.0, 0.0],
                    limit: 10,
                    board_id: Some("b_1".to_owned()),
                    embedding_model: Some("other-test".to_owned()),
                    polarity: None,
                    include_vector: false,
                })
                .unwrap()
                .is_empty(),
            "mismatched embedding model is an empty same-store result"
        );

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

    fn build_chunk(builder: &ChunkBuilder, task_id: &str, title: &str) -> crate::EmbeddingChunk {
        builder
            .build_task_chunks(&TaskChunkSource {
                task_uri: format!("kb://task/{task_id}"),
                project_id: Some("project-a".to_owned()),
                board_id: Some("b_1".to_owned()),
                task_id: task_id.to_owned(),
                title: title.to_owned(),
                description: None,
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
