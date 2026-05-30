
use crate::{Assembler, Result, Tile};

/// Assembles text tiles into a single text document.
pub struct TextAssembler {
    separator: String,
}

impl TextAssembler {
    pub fn new() -> Self {
        Self {
            separator: "\n\n".to_string(),
        }
    }

    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }
}

impl Default for TextAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler for TextAssembler {
    fn assemble(&self, tiles: &[Tile]) -> Result<Vec<u8>> {
        let texts: Vec<String> = tiles
            .iter()
            .filter_map(|t| std::str::from_utf8(&t.payload).ok().map(|s| s.to_string()))
            .collect();

        Ok(texts.join(&self.separator).into_bytes())
    }
}

/// Assembles data row tiles into CSV format.
pub struct CsvAssembler {
    columns: Option<Vec<String>>,
    delimiter: u8,
}

impl CsvAssembler {
    pub fn new() -> Self {
        Self {
            columns: None,
            delimiter: b',',
        }
    }

    pub fn with_columns(mut self, columns: Vec<String>) -> Self {
        self.columns = Some(columns);
        self
    }

    pub fn with_delimiter(mut self, delimiter: u8) -> Self {
        self.delimiter = delimiter;
        self
    }
}

impl Default for CsvAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler for CsvAssembler {
    fn assemble(&self, tiles: &[Tile]) -> Result<Vec<u8>> {
        let mut output = String::new();

        // Determine columns
        let columns: Vec<String> = if let Some(ref cols) = self.columns {
            cols.clone()
        } else {
            // Infer from first tile's metadata
            let mut cols: Vec<String> = tiles
                .first()
                .map(|t| {
                    t.meta
                        .keys()
                        .filter(|k| k.starts_with("col_"))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            cols.sort();
            cols
        };

        // Header
        let header_cols: Vec<&str> = columns.iter().map(|c| c.strip_prefix("col_").unwrap_or(c)).collect();
        output.push_str(&header_cols.to_vec().join(&String::from(self.delimiter as char)));
        output.push('\n');

        // Rows
        for tile in tiles {
            let row: Vec<String> = columns
                .iter()
                .map(|col| tile.meta.get(col).cloned().unwrap_or_default())
                .collect();
            output.push_str(&row.join(&String::from(self.delimiter as char)));
            output.push('\n');
        }

        Ok(output.into_bytes())
    }
}

/// Assembles structured data tiles into a JSON array.
pub struct JsonAssembler {
    wrap_as_object: bool,
}

impl JsonAssembler {
    pub fn new() -> Self {
        Self {
            wrap_as_object: false,
        }
    }

    pub fn as_object(mut self) -> Self {
        self.wrap_as_object = true;
        self
    }
}

impl Default for JsonAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler for JsonAssembler {
    fn assemble(&self, tiles: &[Tile]) -> Result<Vec<u8>> {
        if self.wrap_as_object {
            let mut map = serde_json::Map::new();
            for tile in tiles {
                let key = tile
                    .meta_get("key")
                    .unwrap_or(&tile.index.to_string())
                    .to_string();
                let value: serde_json::Value = serde_json::from_slice(&tile.payload)
                    .unwrap_or(serde_json::Value::String(
                        String::from_utf8_lossy(&tile.payload).to_string(),
                    ));
                map.insert(key, value);
            }
            serde_json::to_vec_pretty(&serde_json::Value::Object(map))
                .map_err(|e| crate::ForgeError::AssemblyFailed(format!("JSON serialization: {e}")))
        } else {
            let values: Vec<serde_json::Value> = tiles
                .iter()
                .map(|tile| {
                    serde_json::from_slice(&tile.payload).unwrap_or(serde_json::Value::String(
                        String::from_utf8_lossy(&tile.payload).to_string(),
                    ))
                })
                .collect();
            serde_json::to_vec_pretty(&values)
                .map_err(|e| crate::ForgeError::AssemblyFailed(format!("JSON serialization: {e}")))
        }
    }
}

/// Assembles text tiles into SRT subtitle format.
pub struct SubtitleAssembler {
    format: SubtitleFormat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SubtitleFormat {
    Srt,
    Vtt,
}

impl SubtitleAssembler {
    pub fn new(format: SubtitleFormat) -> Self {
        Self { format }
    }
}

fn format_timestamp(ms: i64) -> String {
    let h = ms / 3600000;
    let m = (ms % 3600000) / 60000;
    let s = (ms % 60000) / 1000;
    let frac = ms % 1000;
    format!("{h:02}:{m:02}:{s:02},{frac:03}")
}

impl Assembler for SubtitleAssembler {
    fn assemble(&self, tiles: &[Tile]) -> Result<Vec<u8>> {
        let mut output = String::new();

        if self.format == SubtitleFormat::Vtt {
            output.push_str("WEBVTT\n\n");
        }

        for (i, tile) in tiles.iter().enumerate() {
            let start_ms: i64 = tile
                .meta_get("start_ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or((i as i64) * 3000);
            let end_ms: i64 = tile
                .meta_get("end_ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(start_ms + 3000);

            if self.format == SubtitleFormat::Srt {
                output.push_str(&format!("{}\n", i + 1));
            }
            let sep = if self.format == SubtitleFormat::Vtt {
                "."
            } else {
                ","
            };
            let start_fmt = format_timestamp(start_ms).replace(",", sep);
            let end_fmt = format_timestamp(end_ms).replace(",", sep);
            output.push_str(&format!("{start_fmt} --> {end_fmt}\n"));

            if let Some(text) = tile.payload_as_string() {
                output.push_str(text);
            }
            output.push_str("\n\n");
        }

        Ok(output.into_bytes())
    }
}

/// Custom assembler using a user-provided function.
#[allow(clippy::type_complexity)]
pub struct CustomAssembler {
    f: Box<dyn Fn(&[Tile]) -> Result<Vec<u8>> + Send + Sync>,
}

impl CustomAssembler {
    pub fn new(f: impl Fn(&[Tile]) -> Result<Vec<u8>> + Send + Sync + 'static) -> Self {
        Self { f: Box::new(f) }
    }
}

impl Assembler for CustomAssembler {
    fn assemble(&self, tiles: &[Tile]) -> Result<Vec<u8>> {
        (self.f)(tiles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tile, TileKind, Uuid};
    use std::collections::HashMap;

    fn make_text_tile(text: &str, index: u64) -> Tile {
        let source = Uuid::new_v4();
        Tile::new(TileKind::Text, text.as_bytes().to_vec(), source, index)
    }

    fn make_row_tile(row_data: &str, index: u64, meta: HashMap<String, String>) -> Tile {
        let source = Uuid::new_v4();
        let mut tile = Tile::new(TileKind::DataRow, row_data.as_bytes().to_vec(), source, index);
        tile.meta = meta;
        tile
    }

    #[test]
    fn text_assembler_basic() {
        let asm = TextAssembler::new();
        let tiles = vec![
            make_text_tile("Hello", 0),
            make_text_tile("World", 1),
        ];
        let output = asm.assemble(&tiles).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, "Hello\n\nWorld");
    }

    #[test]
    fn text_assembler_custom_separator() {
        let asm = TextAssembler::new().with_separator("\n");
        let tiles = vec![
            make_text_tile("A", 0),
            make_text_tile("B", 1),
        ];
        let output = asm.assemble(&tiles).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "A\nB");
    }

    #[test]
    fn csv_assembler_basic() {
        let asm = CsvAssembler::new().with_columns(vec!["col_name".into(), "col_age".into()]);
        let mut meta0 = HashMap::new();
        meta0.insert("col_name".into(), "alice".into());
        meta0.insert("col_age".into(), "30".into());
        let mut meta1 = HashMap::new();
        meta1.insert("col_name".into(), "bob".into());
        meta1.insert("col_age".into(), "25".into());
        let tiles = vec![
            make_row_tile("alice,30", 0, meta0),
            make_row_tile("bob,25", 1, meta1),
        ];
        let output = asm.assemble(&tiles).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("name"));
        assert!(text.contains("alice"));
        assert!(text.contains("bob"));
    }

    #[test]
    fn json_assembler_array() {
        let asm = JsonAssembler::new();
        let tiles = vec![
            Tile::new(TileKind::StructuredData, b"42".to_vec(), Uuid::new_v4(), 0),
            Tile::new(TileKind::StructuredData, b"99".to_vec(), Uuid::new_v4(), 1),
        ];
        let output = asm.assemble(&tiles).unwrap();
        let val: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(val.as_array().unwrap().len(), 2);
    }

    #[test]
    fn json_assembler_object() {
        let asm = JsonAssembler::new().as_object();
        let mut tile0 = Tile::new(TileKind::StructuredData, b"1".to_vec(), Uuid::new_v4(), 0);
        tile0.meta.insert("key".into(), "x".into());
        let mut tile1 = Tile::new(TileKind::StructuredData, b"2".to_vec(), Uuid::new_v4(), 1);
        tile1.meta.insert("key".into(), "y".into());
        let tiles = vec![tile0, tile1];
        let output = asm.assemble(&tiles).unwrap();
        let val: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(val["x"], 1);
        assert_eq!(val["y"], 2);
    }

    #[test]
    fn subtitle_assembler_srt() {
        let asm = SubtitleAssembler::new(SubtitleFormat::Srt);
        let mut t0 = make_text_tile("Hello", 0);
        t0.meta.insert("start_ms".into(), "1000".into());
        t0.meta.insert("end_ms".into(), "4000".into());
        let mut t1 = make_text_tile("World", 1);
        t1.meta.insert("start_ms".into(), "5000".into());
        t1.meta.insert("end_ms".into(), "8000".into());
        let tiles = vec![t0, t1];
        let output = asm.assemble(&tiles).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("1\n"));
        assert!(text.contains("00:00:01,000 --> 00:00:04,000"));
        assert!(text.contains("Hello"));
        assert!(text.contains("2\n"));
        assert!(text.contains("World"));
    }

    #[test]
    fn subtitle_assembler_vtt() {
        let asm = SubtitleAssembler::new(SubtitleFormat::Vtt);
        let mut t0 = make_text_tile("Hello", 0);
        t0.meta.insert("start_ms".into(), "1000".into());
        t0.meta.insert("end_ms".into(), "3000".into());
        let tiles = vec![t0];
        let output = asm.assemble(&tiles).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("WEBVTT"));
        assert!(text.contains("."));
    }

    #[test]
    fn custom_assembler() {
        let asm = CustomAssembler::new(|tiles| {
            let count = tiles.len();
            Ok(format!("tile count: {count}").into_bytes())
        });
        let tiles = vec![make_text_tile("a", 0), make_text_tile("b", 1)];
        let output = asm.assemble(&tiles).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "tile count: 2");
    }
}
