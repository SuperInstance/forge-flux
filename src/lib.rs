use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod a2a;
pub mod assemblers;
pub mod decomposers;
pub mod memory;
pub mod transforms;

/// Error type for forge-flux operations.
#[derive(Debug)]
pub enum ForgeError {
    DecompositionFailed(String),
    AssemblyFailed(String),
    TransformFailed(String),
    InvalidInput(String),
    MemoryError(String),
    PipelineError(String),
}

impl std::error::Error for ForgeError {}

impl fmt::Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgeError::DecompositionFailed(msg) => write!(f, "decomposition failed: {msg}"),
            ForgeError::AssemblyFailed(msg) => write!(f, "assembly failed: {msg}"),
            ForgeError::TransformFailed(msg) => write!(f, "transform failed: {msg}"),
            ForgeError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            ForgeError::MemoryError(msg) => write!(f, "memory error: {msg}"),
            ForgeError::PipelineError(msg) => write!(f, "pipeline error: {msg}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, ForgeError>;

/// A tile is the atomic unit of agent work.
/// Like a subtitle block, but for ANY data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    /// Tile type (text, audio_chunk, image_region, data_row, etc.)
    pub kind: TileKind,
    /// The tile's content as bytes
    pub payload: Vec<u8>,
    /// Metadata (timestamp, source, language, coordinates, etc.)
    pub meta: HashMap<String, String>,
    /// Tile ID for tracking through the pipeline
    pub id: Uuid,
    /// Source input this was decomposed from
    pub source: Uuid,
    /// Position in the decomposition sequence
    pub index: u64,
    /// Conservation ratio of this tile (how well it preserves information)
    pub cr: f64,
}

impl Tile {
    /// Create a new tile with a random ID and given source.
    pub fn new(kind: TileKind, payload: Vec<u8>, source: Uuid, index: u64) -> Self {
        Self {
            kind,
            payload,
            meta: HashMap::new(),
            id: Uuid::new_v4(),
            source,
            index,
            cr: 1.0,
        }
    }

    /// Add metadata to this tile.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }

    /// Set conservation ratio.
    pub fn with_cr(mut self, cr: f64) -> Self {
        self.cr = cr;
        self
    }

    /// Get payload as UTF-8 string.
    pub fn payload_as_string(&self) -> Option<&str> {
        std::str::from_utf8(&self.payload).ok()
    }

    /// Get a metadata value.
    pub fn meta_get(&self, key: &str) -> Option<&str> {
        self.meta.get(key).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TileKind {
    Text,
    AudioChunk,
    ImageRegion,
    DataRow,
    CodeBlock,
    SensorReading,
    VideoFrame,
    StructuredData,
    Binary,
    Custom(String),
}

impl fmt::Display for TileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TileKind::Text => write!(f, "text"),
            TileKind::AudioChunk => write!(f, "audio_chunk"),
            TileKind::ImageRegion => write!(f, "image_region"),
            TileKind::DataRow => write!(f, "data_row"),
            TileKind::CodeBlock => write!(f, "code_block"),
            TileKind::SensorReading => write!(f, "sensor_reading"),
            TileKind::VideoFrame => write!(f, "video_frame"),
            TileKind::StructuredData => write!(f, "structured_data"),
            TileKind::Binary => write!(f, "binary"),
            TileKind::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

/// Decomposer: breaks any input into tiles.
pub trait Decomposer: Send + Sync {
    /// The tile kind this decomposer produces.
    fn kind(&self) -> TileKind;
    /// Decompose raw input into tiles.
    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>>;
    /// Confidence this decomposer can handle the given input (0.0-1.0).
    fn can_handle(&self, input: &[u8], hint: &str) -> f64;
}

/// Assembler: reassembles tiles into output.
pub trait Assembler: Send + Sync {
    /// Assemble tiles into output bytes.
    fn assemble(&self, tiles: &[Tile]) -> Result<Vec<u8>>;
}

/// Transform: agent operation on tiles (the "flux" part).
pub trait Transform: Send + Sync {
    /// Human-readable name.
    fn name(&self) -> &str;
    /// Apply the transform.
    fn transform(&self, tiles: Vec<Tile>) -> Result<Vec<Tile>>;
    /// Calculate conservation ratio between input and output.
    fn conservation_ratio(&self, input: &[Tile], output: &[Tile]) -> f64 {
        if input.is_empty() {
            return 1.0;
        }
        let input_bytes: usize = input.iter().map(|t| t.payload.len()).sum();
        let output_bytes: usize = output.iter().map(|t| t.payload.len()).sum();
        if input_bytes == 0 {
            return 1.0;
        }
        (output_bytes as f64) / (input_bytes as f64)
    }
}

/// The pipeline: decompose → transform → assemble.
pub struct ForgePipeline {
    decomposer: Box<dyn Decomposer>,
    transforms: Vec<Box<dyn Transform>>,
    assembler: Box<dyn Assembler>,
    source_id: Uuid,
}

impl ForgePipeline {
    /// Create a new pipeline with a decomposer and assembler.
    pub fn new(decomposer: Box<dyn Decomposer>, assembler: Box<dyn Assembler>) -> Self {
        Self {
            decomposer,
            transforms: Vec::new(),
            assembler,
            source_id: Uuid::new_v4(),
        }
    }

    /// Add a transform stage.
    pub fn add_transform(&mut self, transform: Box<dyn Transform>) -> &mut Self {
        self.transforms.push(transform);
        self
    }

    /// Run the full pipeline.
    pub fn run(&self, input: &[u8], meta: HashMap<String, String>) -> Result<Vec<u8>> {
        let tiles = self.decomposer.decompose(input, &meta)?;
        let mut tiles = tiles;

        for transform in &self.transforms {
            tiles = transform.transform(tiles)?;
        }

        self.assembler.assemble(&tiles)
    }

    /// Run but return intermediate tiles for agent inspection.
    /// Returns (final_output, list_of_tile_stages).
    pub fn run_with_tiles(
        &self,
        input: &[u8],
        meta: HashMap<String, String>,
    ) -> Result<(Vec<u8>, Vec<Vec<Tile>>)> {
        let mut stages = Vec::new();

        let tiles = self.decomposer.decompose(input, &meta)?;
        stages.push(tiles.clone());
        let mut tiles = tiles;

        for transform in &self.transforms {
            tiles = transform.transform(tiles)?;
            stages.push(tiles.clone());
        }

        let output = self.assembler.assemble(&tiles)?;
        Ok((output, stages))
    }

    /// Calculate conservation ratio of the full pipeline.
    pub fn conservation_ratio(&self) -> f64 {
        if self.transforms.is_empty() {
            return 1.0;
        }
        // Theoretical max CR; actual depends on input data.
        // Default: product of per-transform CR estimates.
        1.0 // placeholder; real calculation requires running the pipeline
    }

    /// Get the source ID for this pipeline run.
    pub fn source_id(&self) -> Uuid {
        self.source_id
    }

    /// Get the number of transform stages.
    pub fn transform_count(&self) -> usize {
        self.transforms.len()
    }
}

/// Auto-detect the best decomposer for given input.
pub fn detect_decomposer(
    decomposers: &[&dyn Decomposer],
    input: &[u8],
    hint: &str,
) -> Option<usize> {
    let mut best_idx = 0;
    let mut best_score = 0.0f64;

    for (i, dec) in decomposers.iter().enumerate() {
        let score = dec.can_handle(input, hint);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    if best_score > 0.0 {
        Some(best_idx)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_creation() {
        let source = Uuid::new_v4();
        let tile = Tile::new(TileKind::Text, b"hello".to_vec(), source, 0);
        assert_eq!(tile.kind, TileKind::Text);
        assert_eq!(tile.payload, b"hello");
        assert_eq!(tile.index, 0);
        assert_eq!(tile.source, source);
        assert!(!tile.id.is_nil());
    }

    #[test]
    fn tile_serialization() {
        let source = Uuid::new_v4();
        let tile = Tile::new(TileKind::Text, b"test data".to_vec(), source, 0)
            .with_meta("lang", "en")
            .with_cr(0.95);

        let json = serde_json::to_string(&tile).unwrap();
        let back: Tile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, TileKind::Text);
        assert_eq!(back.payload, b"test data");
        assert_eq!(back.meta.get("lang").unwrap(), "en");
        assert!((back.cr - 0.95).abs() < 1e-10);
    }

    #[test]
    fn tile_cloning() {
        let source = Uuid::new_v4();
        let tile = Tile::new(TileKind::Binary, vec![1, 2, 3], source, 0);
        let clone = tile.clone();
        assert_eq!(tile.payload, clone.payload);
        assert_eq!(tile.id, clone.id);
    }

    #[test]
    fn tile_payload_as_string() {
        let source = Uuid::new_v4();
        let tile = Tile::new(TileKind::Text, b"hello world".to_vec(), source, 0);
        assert_eq!(tile.payload_as_string(), Some("hello world"));

        let binary_tile = Tile::new(TileKind::Binary, vec![0xff, 0xfe], source, 0);
        assert!(binary_tile.payload_as_string().is_none());
    }

    #[test]
    fn tile_meta_access() {
        let source = Uuid::new_v4();
        let tile = Tile::new(TileKind::Text, b"data".to_vec(), source, 0)
            .with_meta("key1", "val1")
            .with_meta("key2", "val2");
        assert_eq!(tile.meta_get("key1"), Some("val1"));
        assert_eq!(tile.meta_get("key2"), Some("val2"));
        assert_eq!(tile.meta_get("missing"), None);
    }

    #[test]
    fn tile_kind_display() {
        assert_eq!(TileKind::Text.to_string(), "text");
        assert_eq!(TileKind::AudioChunk.to_string(), "audio_chunk");
        assert_eq!(TileKind::Custom("foo".into()).to_string(), "custom:foo");
    }

    #[test]
    fn error_display() {
        let err = ForgeError::DecompositionFailed("bad input".into());
        assert!(err.to_string().contains("bad input"));
    }

    #[test]
    fn pipeline_new() {
        let p = ForgePipeline::new(
            Box::new(decomposers::TextDecomposer::new()),
            Box::new(assemblers::TextAssembler::new()),
        );
        assert_eq!(p.transform_count(), 0);
    }

    #[test]
    fn pipeline_add_transforms() {
        let mut p = ForgePipeline::new(
            Box::new(decomposers::TextDecomposer::new()),
            Box::new(assemblers::TextAssembler::new()),
        );
        p.add_transform(Box::new(transforms::SortTransform::new(|a, b| {
            a.payload.cmp(&b.payload)
        })));
        assert_eq!(p.transform_count(), 1);
    }

    #[test]
    fn detect_decomposer_text() {
        let text_dec = decomposers::TextDecomposer::new();
        let csv_dec = decomposers::CsvDecomposer::new();
        let decs: Vec<&dyn Decomposer> = vec![&text_dec, &csv_dec];
        let idx = detect_decomposer(&decs, b"hello world\nhow are you", "text");
        assert_eq!(idx, Some(0)); // text decomposer should win
    }

    #[test]
    fn detect_decomposer_csv() {
        let text_dec = decomposers::TextDecomposer::new();
        let csv_dec = decomposers::CsvDecomposer::new();
        let decs: Vec<&dyn Decomposer> = vec![&text_dec, &csv_dec];
        let idx = detect_decomposer(&decs, b"name,age\nalice,30\nbob,25", "csv");
        assert_eq!(idx, Some(1)); // csv decomposer should win
    }
}
