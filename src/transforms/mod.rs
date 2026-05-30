use crate::{Result, Tile, Transform};

/// Translate transform — placeholder that marks tiles as translated.
/// In production, this would call a translation API.
pub struct TranslateTransform {
    target_lang: String,
}

impl TranslateTransform {
    pub fn new(target_lang: impl Into<String>) -> Self {
        Self {
            target_lang: target_lang.into(),
        }
    }
}

impl Transform for TranslateTransform {
    fn name(&self) -> &str {
        "translate"
    }

    fn transform(&self, mut tiles: Vec<Tile>) -> Result<Vec<Tile>> {
        for tile in &mut tiles {
            tile.meta
                .insert("translated_to".to_string(), self.target_lang.clone());
            // Mark as translated (actual translation would happen here)
            tile.meta.insert("translation_status".to_string(), "completed".to_string());
        }
        Ok(tiles)
    }
}

/// Summarize transform — reduces tiles by merging consecutive ones.
pub struct SummarizeTransform {
    ratio: f64, // target ratio of output tiles to input tiles
}

impl SummarizeTransform {
    pub fn new(ratio: f64) -> Self {
        Self { ratio: ratio.clamp(0.01, 1.0) }
    }
}

impl Transform for SummarizeTransform {
    fn name(&self) -> &str {
        "summarize"
    }

    fn transform(&self, tiles: Vec<Tile>) -> Result<Vec<Tile>> {
        if tiles.is_empty() {
            return Ok(tiles);
        }

        let target_count = ((tiles.len() as f64) * self.ratio).ceil() as usize;
        let target_count = target_count.max(1);

        let chunk_size = tiles.len().div_ceil(target_count);
        let mut result = Vec::with_capacity(target_count);

        for (i, chunk) in tiles.chunks(chunk_size).enumerate() {
            let combined_payload: Vec<u8> = chunk
                .iter()
                .flat_map(|t| {
                    let mut v = t.payload.clone();
                    v.push(b'\n');
                    v
                })
                .collect();

            let source = chunk.first().map(|t| t.source).unwrap_or_default();
            let mut tile = Tile::new(chunk[0].kind.clone(), combined_payload, source, i as u64);
            tile.meta.insert(
                "summarized_from".to_string(),
                chunk.len().to_string(),
            );
            tile.cr = self.ratio;
            result.push(tile);
        }

        Ok(result)
    }
}

/// Filter transform — removes tiles matching a predicate.
pub struct FilterTransform {
    predicate: Box<dyn Fn(&Tile) -> bool + Send + Sync>,
}

impl FilterTransform {
    pub fn new(predicate: impl Fn(&Tile) -> bool + Send + Sync + 'static) -> Self {
        Self {
            predicate: Box::new(predicate),
        }
    }
}

impl Transform for FilterTransform {
    fn name(&self) -> &str {
        "filter"
    }

    fn transform(&self, tiles: Vec<Tile>) -> Result<Vec<Tile>> {
        let filtered: Vec<Tile> = tiles
            .into_iter()
            .filter(|t| (self.predicate)(t))
            .enumerate()
            .map(|(i, mut t)| {
                t.index = i as u64;
                t
            })
            .collect();
        Ok(filtered)
    }
}

#[allow(clippy::type_complexity)]
/// Sort transform — reorders tiles using a comparator.
pub struct SortTransform {
    compare: Box<dyn Fn(&Tile, &Tile) -> std::cmp::Ordering + Send + Sync>,
}

impl SortTransform {
    pub fn new(compare: impl Fn(&Tile, &Tile) -> std::cmp::Ordering + Send + Sync + 'static) -> Self {
        Self {
            compare: Box::new(compare),
        }
    }
}

impl Transform for SortTransform {
    fn name(&self) -> &str {
        "sort"
    }

    fn transform(&self, mut tiles: Vec<Tile>) -> Result<Vec<Tile>> {
        tiles.sort_by(|a, b| (self.compare)(a, b));
        for (i, tile) in tiles.iter_mut().enumerate() {
            tile.index = i as u64;
        }
        Ok(tiles)
    }
}

/// Map transform — applies a function to each tile's payload.
#[allow(clippy::type_complexity)]
pub struct MapTransform {
    f: Box<dyn Fn(&Tile) -> Vec<u8> + Send + Sync>,
}

impl MapTransform {
    pub fn new(f: impl Fn(&Tile) -> Vec<u8> + Send + Sync + 'static) -> Self {
        Self { f: Box::new(f) }
    }
}

impl Transform for MapTransform {
    fn name(&self) -> &str {
        "map"
    }

    fn transform(&self, tiles: Vec<Tile>) -> Result<Vec<Tile>> {
        tiles
            .into_iter()
            .map(|mut tile| {
                tile.payload = (self.f)(&tile);
                Ok(tile)
            })
            .collect()
    }
}

/// Conservation transform — verifies CR and enforces minimum.
pub struct ConservationTransform {
    min_cr: f64,
}

impl ConservationTransform {
    pub fn new(min_cr: f64) -> Self {
        Self {
            min_cr: min_cr.clamp(0.0, 1.0),
        }
    }
}

impl Transform for ConservationTransform {
    fn name(&self) -> &str {
        "conservation"
    }

    fn transform(&self, mut tiles: Vec<Tile>) -> Result<Vec<Tile>> {
        for tile in &mut tiles {
            if tile.cr < self.min_cr {
                tile.meta.insert(
                    "conservation_warning".to_string(),
                    format!("CR {:.3} below minimum {:.3}", tile.cr, self.min_cr),
                );
            }
            tile.meta.insert("conservation_checked".to_string(), "true".to_string());
        }
        Ok(tiles)
    }

    fn conservation_ratio(&self, input: &[Tile], output: &[Tile]) -> f64 {
        if input.is_empty() {
            return 1.0;
        }
        let avg_cr: f64 = output.iter().map(|t| t.cr).sum::<f64>() / output.len() as f64;
        avg_cr
    }
}

/// Flux transform — converts tiles between TileKinds.
pub struct FluxTransform {
    source_kind: crate::TileKind,
    target_kind: crate::TileKind,
}

impl FluxTransform {
    pub fn new(source_kind: crate::TileKind, target_kind: crate::TileKind) -> Self {
        Self {
            source_kind,
            target_kind,
        }
    }
}

impl Transform for FluxTransform {
    fn name(&self) -> &str {
        "flux"
    }

    fn transform(&self, mut tiles: Vec<Tile>) -> Result<Vec<Tile>> {
        for tile in &mut tiles {
            if tile.kind == self.source_kind {
                tile.kind = self.target_kind.clone();
                tile.meta.insert(
                    "flux_from".to_string(),
                    self.source_kind.to_string(),
                );
                tile.meta.insert(
                    "flux_to".to_string(),
                    self.target_kind.to_string(),
                );
            }
        }
        Ok(tiles)
    }
}

// Need Tile import for constructors

