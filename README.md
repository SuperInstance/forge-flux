# forge-flux

**Generalized input decomposition for agent pipelines.**

```
Input → Decomposer → Tiles → Agent Transform → Tiles → Assembler → Output
```

## The Idea

SubForge decomposed media into subtitle tiles through a pipeline: extract → transcribe → translate → mux. **ForgeFlux generalizes this concept**: ANY input gets decomposed into tiles that agents can read, transform, and reassemble.

A **tile** is the atomic unit of agent work. It's what gets posted as a tick, read by agents, and transformed algorithmically.

## The Math is Conservation

Every tile carries a **conservation ratio** (CR) — a measure of how well it preserves information through transformation. The pipeline tracks CR at every stage:

```
CR(tiles_out) = Σ |payload_out| / Σ |payload_in|
```

Plato agents subscribe to ForgeTicks and can inject transforms mid-pipeline to maintain conservation.

## Quick Start

```rust
use forge_flux::*;
use std::collections::HashMap;

// Build a pipeline
let mut pipeline = ForgePipeline::new(
    Box::new(decomposers::TextDecomposer::new()),
    Box::new(assemblers::TextAssembler::new()),
);

// Add transforms
pipeline.add_transform(Box::new(transforms::SortTransform::new(|a, b| {
    a.payload.cmp(&b.payload)
})));

// Run it
let output = pipeline.run(b"cherry\n\napple\n\nbanana", HashMap::new()).unwrap();
assert_eq!(String::from_utf8(output).unwrap(), "apple\n\nbanana\n\ncherry");
```

## Tile

```rust
pub struct Tile {
    pub kind: TileKind,        // Text, AudioChunk, ImageRegion, DataRow, ...
    pub payload: Vec<u8>,      // The actual data
    pub meta: HashMap<String, String>,  // Metadata
    pub id: Uuid,              // Tracking ID
    pub source: Uuid,          // Source input
    pub index: u64,            // Position in sequence
    pub cr: f64,               // Conservation ratio
}
```

## Built-in Decomposers

| Decomposer | Input | Tile Kind |
|---|---|---|
| `TextDecomposer` | Text files | `Text` |
| `CsvDecomposer` | CSV data | `DataRow` |
| `JsonDecomposer` | JSON | `StructuredData` |
| `SubtitleDecomposer` | SRT/VTT | `Text` |
| `CodeDecomposer` | Source code | `CodeBlock` |
| `AudioDecomposer` | Raw audio | `AudioChunk` |
| `ImageDecomposer` | Images | `ImageRegion` |
| `SensorDecomposer` | Sensor data | `SensorReading` |

## Built-in Transforms

| Transform | What it does |
|---|---|
| `TranslateTransform` | Marks tiles for translation |
| `SummarizeTransform` | Reduces tile count by merging |
| `FilterTransform` | Removes tiles matching predicate |
| `SortTransform` | Reorders tiles |
| `MapTransform` | Applies function to each tile |
| `ConservationTransform` | Verifies CR thresholds |
| `FluxTransform` | Converts between TileKinds |

## Built-in Assemblers

| Assembler | Output |
|---|---|
| `TextAssembler` | Text document |
| `CsvAssembler` | CSV |
| `JsonAssembler` | JSON array/object |
| `SubtitleAssembler` | SRT or VTT |
| `CustomAssembler` | User-defined |

## A2A Integration

The pipeline emits `ForgeTick` messages compatible with A2A (agent-to-agent) protocols:

```rust
pub struct ForgeTick {
    pub pipeline_id: Uuid,
    pub stage: String,       // "decompose", "transform", "assemble"
    pub tiles_in: u64,
    pub tiles_out: u64,
    pub cr: f64,
    pub timestamp: i64,
}
```

Plato agents subscribe to ForgeTicks and can inject transforms mid-pipeline.

## External Memory

Tiles can be stored and retrieved via the `TileMemory` trait:

```rust
let memory = InMemoryTileMemory::new();
let ids = memory.store(&tiles).unwrap();
let retrieved = memory.retrieve(&ids).unwrap();
let results = memory.search("hello", Some(TileKind::Text), 10).unwrap();
```

## The SubForge Pattern

The original use case — subtitle translation — is a 3-line pipeline:

```rust
let mut p = ForgePipeline::new(
    Box::new(decomposers::SubtitleDecomposer::new()),
    Box::new(assemblers::SubtitleAssembler::new(SubtitleFormat::Srt)),
);
p.add_transform(Box::new(transforms::TranslateTransform::new("es")));
let output = p.run(srt_bytes, HashMap::new()).unwrap();
```

## Dependencies

- `serde` + `serde_json` — serialization
- `uuid` — tile/pipeline identification

That's it. Zero heavy dependencies.

## License

MIT
