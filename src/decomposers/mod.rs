use std::collections::HashMap;

use crate::{Decomposer, Result, Tile, TileKind, Uuid};

/// Decomposes text into tiles split by paragraph (double newline) or sentence.
pub struct TextDecomposer {
    split_mode: TextSplitMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextSplitMode {
    Paragraph,
    Sentence,
    Line,
}

impl TextDecomposer {
    pub fn new() -> Self {
        Self {
            split_mode: TextSplitMode::Paragraph,
        }
    }

    pub fn with_mode(mut self, mode: TextSplitMode) -> Self {
        self.split_mode = mode;
        self
    }
}

impl Default for TextDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

impl Decomposer for TextDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::Text
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        let text = std::str::from_utf8(input)
            .map_err(|e| crate::ForgeError::DecompositionFailed(format!("invalid UTF-8: {e}")))?;

        let source = Uuid::new_v4();
        let chunks: Vec<&str> = match self.split_mode {
            TextSplitMode::Paragraph => text
                .split("\n\n")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect(),
            TextSplitMode::Sentence => text
                .split_inclusive(|c: char| c == '.' || c == '!' || c == '?')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect(),
            TextSplitMode::Line => text
                .lines()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect(),
        };

        let tiles = chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| {
                let mut tile = Tile::new(TileKind::Text, chunk.as_bytes().to_vec(), source, i as u64);
                // Inherit relevant meta
                for (k, v) in meta {
                    if k.starts_with("text_") || k == "lang" || k == "source" {
                        tile.meta.insert(k.clone(), v.clone());
                    }
                }
                tile
            })
            .collect();

        Ok(tiles)
    }

    fn can_handle(&self, input: &[u8], hint: &str) -> f64 {
        if hint == "text" || hint == "txt" {
            return 0.9;
        }
        // Check if input is valid UTF-8
        if std::str::from_utf8(input).is_ok() {
            return 0.5;
        }
        0.0
    }
}

/// Decomposes CSV data into row tiles.
pub struct CsvDecomposer {
    has_headers: bool,
    delimiter: u8,
}

impl CsvDecomposer {
    pub fn new() -> Self {
        Self {
            has_headers: true,
            delimiter: b',',
        }
    }

    pub fn with_headers(mut self, has_headers: bool) -> Self {
        self.has_headers = has_headers;
        self
    }

    pub fn with_delimiter(mut self, delimiter: u8) -> Self {
        self.delimiter = delimiter;
        self
    }
}

impl Default for CsvDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

impl Decomposer for CsvDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::DataRow
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        let text = std::str::from_utf8(input)
            .map_err(|e| crate::ForgeError::DecompositionFailed(format!("invalid UTF-8: {e}")))?;

        let source = Uuid::new_v4();
        let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();

        let headers: Option<Vec<String>> = if self.has_headers && !lines.is_empty() {
            let header_line = lines.remove(0);
            Some(
                header_line
                    .split(self.delimiter as char)
                    .map(|s| s.trim().to_string())
                    .collect(),
            )
        } else {
            None
        };

        let tiles = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let values: Vec<&str> = line
                    .split(self.delimiter as char)
                    .map(|s| s.trim())
                    .collect();

                let mut tile = Tile::new(TileKind::DataRow, line.as_bytes().to_vec(), source, i as u64);

                if let Some(ref h) = headers {
                    for (j, val) in values.iter().enumerate() {
                        if j < h.len() {
                            tile.meta.insert(format!("col_{}", h[j]), val.to_string());
                        }
                    }
                    tile.meta.insert("columns".to_string(), h.join(","));
                }
                tile.meta.insert("row".to_string(), i.to_string());
                for (k, v) in meta {
                    if k.starts_with("csv_") || k == "source" {
                        tile.meta.insert(k.clone(), v.clone());
                    }
                }
                tile
            })
            .collect();

        Ok(tiles)
    }

    fn can_handle(&self, input: &[u8], hint: &str) -> f64 {
        if hint == "csv" {
            return 0.95;
        }
        if let Ok(text) = std::str::from_utf8(input) {
            let first_line = text.lines().next().unwrap_or("");
            if first_line.contains(',') {
                let comma_count = first_line.matches(',').count();
                if comma_count > 0 {
                    let second_line = text.lines().nth(1).unwrap_or("");
                    if second_line.matches(',').count() == comma_count {
                        return 0.85;
                    }
                }
            }
        }
        0.0
    }
}

/// Decomposes JSON into tiles for each object/array element.
pub struct JsonDecomposer;

impl JsonDecomposer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

impl Decomposer for JsonDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::StructuredData
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        let value: serde_json::Value = serde_json::from_slice(input).map_err(|e| {
            crate::ForgeError::DecompositionFailed(format!("invalid JSON: {e}"))
        })?;

        let source = Uuid::new_v4();

        let tiles = match value {
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    let mut tile = Tile::new(
                        TileKind::StructuredData,
                        serde_json::to_vec(&v).unwrap_or_default(),
                        source,
                        i as u64,
                    );
                    tile.meta.insert("path".to_string(), format!("[{i}]"));
                    tile
                })
                .collect(),
            serde_json::Value::Object(map) => map
                .into_iter()
                .enumerate()
                .map(|(i, (key, v))| {
                    let mut tile = Tile::new(
                        TileKind::StructuredData,
                        serde_json::to_vec(&v).unwrap_or_default(),
                        source,
                        i as u64,
                    );
                    tile.meta.insert("key".to_string(), key.clone());
                    tile.meta.insert("path".to_string(), format!(".{key}"));
                    tile
                })
                .collect(),
            other => {
                // Single value — one tile
                vec![{
                    let mut tile = Tile::new(
                        TileKind::StructuredData,
                        serde_json::to_vec(&other).unwrap_or_default(),
                        source,
                        0,
                    );
                    tile.meta.insert("path".to_string(), "$".to_string());
                    tile
                }]
            }
        };

        Ok(tiles)
    }

    fn can_handle(&self, input: &[u8], hint: &str) -> f64 {
        if hint == "json" {
            return 0.95;
        }
        let trimmed = input.iter().skip_while(|&&b| b == b' ' || b == b'\n' || b == b'\r');
        let first = *trimmed.clone().next().unwrap_or(&0);
        if first == b'{' || first == b'[' {
            return 0.8;
        }
        0.0
    }
}

/// Decomposes subtitle files (SRT/VTT) into tiles.
pub struct SubtitleDecomposer;

impl SubtitleDecomposer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SubtitleDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_timestamp(ts: &str) -> Option<i64> {
    // Format: HH:MM:SS,mmm or HH:MM:SS.mmm
    let ts = ts.trim();
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: i64 = parts[0].parse().ok()?;
    let m: i64 = parts[1].parse().ok()?;
    let sec_part = parts[2];
    let sec_parts: Vec<&str> = sec_part.split(|c| c == ',' || c == '.').collect();
    let s: i64 = sec_parts.first()?.parse().ok()?;
    let ms: i64 = if sec_parts.len() > 1 {
        let frac = sec_parts[1];
        let frac = if frac.len() == 2 {
            format!("{frac}0")
        } else if frac.len() == 1 {
            format!("{frac}00")
        } else {
            frac.to_string()
        };
        frac.parse().unwrap_or(0)
    } else {
        0
    };
    Some(h * 3600000 + m * 60000 + s * 1000 + ms)
}

impl Decomposer for SubtitleDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::Text
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        let text = std::str::from_utf8(input)
            .map_err(|e| crate::ForgeError::DecompositionFailed(format!("invalid UTF-8: {e}")))?;

        let source = Uuid::new_v4();
        // Strip WEBVTT header if present
        let text = text.strip_prefix("WEBVTT").unwrap_or(text);
        let text = text.trim_start_matches(|c: char| c == '\n' || c == '\r');

        // Split by blank lines to get blocks
        let blocks: Vec<&str> = text
            .split("\n\n")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut tiles = Vec::new();
        let mut idx = 0u64;

        for block in blocks {
            let lines: Vec<&str> = block.lines().collect();
            if lines.is_empty() {
                continue;
            }

            // Find the timestamp line (contains -->)
            let ts_line_idx = lines.iter().position(|l| l.contains("-->"));

            if let Some(ti) = ts_line_idx {
                let ts_line = lines[ti];
                let ts_parts: Vec<&str> = ts_line.split("-->").collect();
                let start_ms = ts_parts
                    .first()
                    .and_then(|s| parse_timestamp(s.trim()))
                    .unwrap_or(0);
                let end_ms = ts_parts
                    .get(1)
                    .and_then(|s| parse_timestamp(s.trim()))
                    .unwrap_or(0);

                let text_lines: Vec<&str> = if ti > 0 {
                    // First line might be sequence number
                    lines[ti + 1..].to_vec()
                } else {
                    lines[ti + 1..].to_vec()
                };

                let subtitle_text = text_lines.join("\n").trim().to_string();
                if subtitle_text.is_empty() {
                    continue;
                }

                let mut tile = Tile::new(TileKind::Text, subtitle_text.as_bytes().to_vec(), source, idx);
                tile.meta
                    .insert("start_ms".to_string(), start_ms.to_string());
                tile.meta.insert("end_ms".to_string(), end_ms.to_string());
                tile.meta
                    .insert("duration_ms".to_string(), (end_ms - start_ms).to_string());
                tile.meta
                    .insert("subtitle_index".to_string(), idx.to_string());
                for (k, v) in meta {
                    if k.starts_with("sub_") || k == "lang" || k == "source" {
                        tile.meta.insert(k.clone(), v.clone());
                    }
                }
                tiles.push(tile);
                idx += 1;
            }
        }

        Ok(tiles)
    }

    fn can_handle(&self, input: &[u8], hint: &str) -> f64 {
        if hint == "srt" || hint == "vtt" {
            return 0.95;
        }
        if let Ok(text) = std::str::from_utf8(input) {
            if text.contains("-->") && text.contains(':') {
                return 0.7;
            }
            if text.starts_with("WEBVTT") {
                return 0.9;
            }
        }
        0.0
    }
}

/// Decomposes code into tiles split by functions/blocks.
pub struct CodeDecomposer {
    language: String,
}

impl CodeDecomposer {
    pub fn new(language: impl Into<String>) -> Self {
        Self {
            language: language.into(),
        }
    }
}

impl Decomposer for CodeDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::CodeBlock
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        let text = std::str::from_utf8(input)
            .map_err(|e| crate::ForgeError::DecompositionFailed(format!("invalid UTF-8: {e}")))?;

        let source = Uuid::new_v4();

        // Simple heuristic: split on double newlines or brace-level boundaries
        let mut tiles = Vec::new();
        let mut idx = 0u64;
        let mut current_block = String::new();
        let mut brace_depth = 0i32;
        let mut block_start_line = 0usize;
        let mut line_num = 0usize;

        for line in text.lines() {
            let trimmed = line.trim();
            line_num += 1;

            // Detect function/method/struct/class definitions
            let is_definition = trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("function ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("pub impl ");

            let open_braces = trimmed.matches('{').count() as i32;
            let close_braces = trimmed.matches('}').count() as i32;

            if is_definition && brace_depth == 0 && !current_block.trim().is_empty() {
                // Save previous block
                let payload = current_block.trim().as_bytes().to_vec();
                let mut tile = Tile::new(TileKind::CodeBlock, payload, source, idx);
                tile.meta
                    .insert("language".to_string(), self.language.clone());
                tile.meta
                    .insert("start_line".to_string(), block_start_line.to_string());
                tile.meta.insert(
                    "end_line".to_string(),
                    (line_num.saturating_sub(1)).to_string(),
                );
                for (k, v) in meta {
                    if k.starts_with("code_") || k == "source" {
                        tile.meta.insert(k.clone(), v.clone());
                    }
                }
                tiles.push(tile);
                idx += 1;
                current_block.clear();
                block_start_line = line_num;
            }

            if current_block.is_empty() {
                block_start_line = line_num;
            }
            current_block.push_str(line);
            current_block.push('\n');

            brace_depth += open_braces - close_braces;

            if brace_depth <= 0 && !current_block.trim().is_empty() {
                brace_depth = 0;
            }
        }

        // Final block
        if !current_block.trim().is_empty() {
            let payload = current_block.trim().as_bytes().to_vec();
            let mut tile = Tile::new(TileKind::CodeBlock, payload, source, idx);
            tile.meta
                .insert("language".to_string(), self.language.clone());
            tile.meta
                .insert("start_line".to_string(), block_start_line.to_string());
            tile.meta
                .insert("end_line".to_string(), line_num.to_string());
            tiles.push(tile);
        }

        Ok(tiles)
    }

    fn can_handle(&self, input: &[u8], hint: &str) -> f64 {
        if hint == "code" || hint == "rust" || hint == "python" || hint == "js" {
            return 0.9;
        }
        if let Ok(text) = std::str::from_utf8(input) {
            if text.contains("fn ") || text.contains("def ") || text.contains("function ") {
                return 0.6;
            }
        }
        0.0
    }
}

/// Decomposes audio into fixed-duration chunk tiles (placeholder — needs real audio parsing).
pub struct AudioDecomposer {
    chunk_duration_ms: u64,
}

impl AudioDecomposer {
    pub fn new(chunk_duration_ms: u64) -> Self {
        Self { chunk_duration_ms }
    }
}

impl Default for AudioDecomposer {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl Decomposer for AudioDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::AudioChunk
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        // Placeholder: split raw bytes into equal chunks
        let source = Uuid::new_v4();
        let chunk_size = (meta
            .get("sample_rate")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(16000)
            / 1000
            * self.chunk_duration_ms as usize)
            .max(1);

        let tiles: Vec<Tile> = input
            .chunks(chunk_size)
            .enumerate()
            .map(|(i, chunk)| {
                let mut tile = Tile::new(TileKind::AudioChunk, chunk.to_vec(), source, i as u64);
                tile.meta.insert(
                    "start_ms".to_string(),
                    (i as u64 * self.chunk_duration_ms).to_string(),
                );
                tile.meta.insert(
                    "duration_ms".to_string(),
                    self.chunk_duration_ms.to_string(),
                );
                tile
            })
            .collect();

        Ok(tiles)
    }

    fn can_handle(&self, _input: &[u8], hint: &str) -> f64 {
        if hint == "audio" || hint == "wav" || hint == "mp3" {
            return 0.7;
        }
        0.0
    }
}

/// Decomposes image into region tiles (placeholder — needs real image parsing).
pub struct ImageDecomposer {
    tile_width: u32,
    tile_height: u32,
}

impl ImageDecomposer {
    pub fn new(tile_width: u32, tile_height: u32) -> Self {
        Self {
            tile_width,
            tile_height,
        }
    }
}

impl Default for ImageDecomposer {
    fn default() -> Self {
        Self::new(64, 64)
    }
}

impl Decomposer for ImageDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::ImageRegion
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        // Placeholder: produce a single tile with the whole image
        let source = Uuid::new_v4();
        let width = meta
            .get("width")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let height = meta
            .get("height")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let mut tile = Tile::new(TileKind::ImageRegion, input.to_vec(), source, 0);
        tile.meta
            .insert("tile_width".to_string(), self.tile_width.to_string());
        tile.meta
            .insert("tile_height".to_string(), self.tile_height.to_string());
        if width > 0 {
            tile.meta.insert("width".to_string(), width.to_string());
        }
        if height > 0 {
            tile.meta.insert("height".to_string(), height.to_string());
        }
        tile.meta.insert("x".to_string(), "0".to_string());
        tile.meta.insert("y".to_string(), "0".to_string());

        Ok(vec![tile])
    }

    fn can_handle(&self, _input: &[u8], hint: &str) -> f64 {
        if hint == "image" || hint == "png" || hint == "jpg" || hint == "jpeg" {
            return 0.7;
        }
        0.0
    }
}

/// Decomposes sensor readings into tiles.
pub struct SensorDecomposer;

impl SensorDecomposer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SensorDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

impl Decomposer for SensorDecomposer {
    fn kind(&self) -> TileKind {
        TileKind::SensorReading
    }

    fn decompose(&self, input: &[u8], meta: &HashMap<String, String>) -> Result<Vec<Tile>> {
        // Expects newline-separated readings: "timestamp,value" or just "value"
        let text = std::str::from_utf8(input)
            .map_err(|e| crate::ForgeError::DecompositionFailed(format!("invalid UTF-8: {e}")))?;

        let source = Uuid::new_v4();
        let tiles: Vec<Tile> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, line)| {
                let mut tile =
                    Tile::new(TileKind::SensorReading, line.trim().as_bytes().to_vec(), source, i as u64);
                let parts: Vec<&str> = line.splitn(2, ',').collect();
                if parts.len() == 2 {
                    tile.meta
                        .insert("timestamp".to_string(), parts[0].trim().to_string());
                    tile.meta
                        .insert("value".to_string(), parts[1].trim().to_string());
                } else {
                    tile.meta
                        .insert("value".to_string(), line.trim().to_string());
                }
                for (k, v) in meta {
                    if k.starts_with("sensor_") || k == "source" {
                        tile.meta.insert(k.clone(), v.clone());
                    }
                }
                tile
            })
            .collect();

        Ok(tiles)
    }

    fn can_handle(&self, _input: &[u8], hint: &str) -> f64 {
        if hint == "sensor" || hint == "telemetry" || hint == "readings" {
            return 0.8;
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_decomposer_paragraph() {
        let dec = TextDecomposer::new();
        let input = b"First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0].payload_as_string(), Some("First paragraph."));
        assert_eq!(tiles[1].payload_as_string(), Some("Second paragraph."));
    }

    #[test]
    fn text_decomposer_sentence() {
        let dec = TextDecomposer::new().with_mode(TextSplitMode::Sentence);
        let input = b"Hello world. How are you? Fine!";
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 3);
    }

    #[test]
    fn text_decomposer_empty() {
        let dec = TextDecomposer::new();
        let tiles = dec.decompose(b"", &HashMap::new()).unwrap();
        assert!(tiles.is_empty());
    }

    #[test]
    fn csv_decomposer_basic() {
        let dec = CsvDecomposer::new();
        let input = b"name,age\nalice,30\nbob,25";
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].meta_get("col_name"), Some("alice"));
        assert_eq!(tiles[0].meta_get("col_age"), Some("30"));
        assert_eq!(tiles[1].meta_get("col_name"), Some("bob"));
    }

    #[test]
    fn csv_decomposer_confidence() {
        let dec = CsvDecomposer::new();
        assert!(dec.can_handle(b"a,b\nc,d", "") > 0.5);
        assert!(dec.can_handle(b"hello world", "text") < 0.1);
    }

    #[test]
    fn json_decomposer_array() {
        let dec = JsonDecomposer::new();
        let input = br#"[{"name":"a"},{"name":"b"}]"#;
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].meta_get("path"), Some("[0]"));
    }

    #[test]
    fn json_decomposer_object() {
        let dec = JsonDecomposer::new();
        let input = br#"{"x":1,"y":2}"#;
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 2);
    }

    #[test]
    fn json_decomposer_invalid() {
        let dec = JsonDecomposer::new();
        let result = dec.decompose(b"not json", &HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn subtitle_decomposer_srt() {
        let dec = SubtitleDecomposer::new();
        let input = b"1\n00:00:01,000 --> 00:00:04,000\nHello world\n\n2\n00:00:05,000 --> 00:00:08,000\nHow are you?";
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].payload_as_string(), Some("Hello world"));
        assert_eq!(tiles[0].meta_get("start_ms"), Some("1000"));
        assert_eq!(tiles[0].meta_get("end_ms"), Some("4000"));
        assert_eq!(tiles[1].payload_as_string(), Some("How are you?"));
    }

    #[test]
    fn subtitle_decomposer_vtt() {
        let dec = SubtitleDecomposer::new();
        let input = b"WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nHello";
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].payload_as_string(), Some("Hello"));
    }

    #[test]
    fn code_decomposer_rust() {
        let dec = CodeDecomposer::new("rust");
        let input = b"fn main() {\n    println!(\"hello\");\n}\n\nfn other() {\n    42\n}";
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert!(tiles.len() >= 1);
        assert_eq!(tiles[0].meta_get("language"), Some("rust"));
    }

    #[test]
    fn sensor_decomposer() {
        let dec = SensorDecomposer::new();
        let input = b"1000,23.5\n2000,24.0\n3000,22.1";
        let tiles = dec.decompose(input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0].meta_get("timestamp"), Some("1000"));
        assert_eq!(tiles[0].meta_get("value"), Some("23.5"));
    }

    #[test]
    fn audio_decomposer_placeholder() {
        let dec = AudioDecomposer::new(500);
        let input = vec![0u8; 100];
        let tiles = dec.decompose(&input, &HashMap::new()).unwrap();
        assert!(!tiles.is_empty());
        assert_eq!(tiles[0].kind, TileKind::AudioChunk);
    }

    #[test]
    fn image_decomposer_placeholder() {
        let dec = ImageDecomposer::new(32, 32);
        let input = vec![0u8; 100];
        let tiles = dec.decompose(&input, &HashMap::new()).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].kind, TileKind::ImageRegion);
    }
}
