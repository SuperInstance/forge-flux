use uuid::Uuid;

use crate::{Result, Tile, TileKind};

/// External memory interface for tiles.
pub trait TileMemory: Send + Sync {
    fn store(&self, tiles: &[Tile]) -> Result<Vec<Uuid>>;
    fn retrieve(&self, ids: &[Uuid]) -> Result<Vec<Tile>>;
    fn search(&self, query: &str, kind: Option<TileKind>, limit: usize) -> Result<Vec<Tile>>;
}

/// In-memory tile store for testing and simple use cases.
pub struct InMemoryTileMemory {
    tiles: std::sync::Mutex<std::collections::HashMap<Uuid, Tile>>,
}

impl InMemoryTileMemory {
    pub fn new() -> Self {
        Self {
            tiles: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryTileMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl TileMemory for InMemoryTileMemory {
    fn store(&self, tiles: &[Tile]) -> Result<Vec<Uuid>> {
        let mut store = self.tiles.lock().unwrap();
        let ids: Vec<Uuid> = tiles.iter().map(|t| t.id).collect();
        for tile in tiles {
            store.insert(tile.id, tile.clone());
        }
        Ok(ids)
    }

    fn retrieve(&self, ids: &[Uuid]) -> Result<Vec<Tile>> {
        let store = self.tiles.lock().unwrap();
        let tiles: Vec<Tile> = ids
            .iter()
            .filter_map(|id| store.get(id).cloned())
            .collect();
        Ok(tiles)
    }

    fn search(&self, query: &str, kind: Option<TileKind>, limit: usize) -> Result<Vec<Tile>> {
        let store = self.tiles.lock().unwrap();
        let query_lower = query.to_lowercase();
        let results: Vec<Tile> = store
            .values()
            .filter(|tile| {
                if let Some(ref k) = kind {
                    if tile.kind != *k {
                        return false;
                    }
                }
                // Search in payload (UTF-8) and metadata values
                let payload_match = std::str::from_utf8(&tile.payload)
                    .map(|s| s.to_lowercase().contains(&query_lower))
                    .unwrap_or(false);
                let meta_match = tile
                    .meta
                    .values()
                    .any(|v| v.to_lowercase().contains(&query_lower));
                payload_match || meta_match
            })
            .take(limit)
            .cloned()
            .collect();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn store_and_retrieve() {
        let mem = InMemoryTileMemory::new();
        let source = Uuid::new_v4();
        let tiles = vec![
            Tile::new(TileKind::Text, b"hello world".to_vec(), source, 0),
            Tile::new(TileKind::Text, b"foo bar".to_vec(), source, 1),
        ];
        let ids = mem.store(&tiles).unwrap();
        assert_eq!(ids.len(), 2);

        let retrieved = mem.retrieve(&ids).unwrap();
        assert_eq!(retrieved.len(), 2);
    }

    #[test]
    fn retrieve_missing() {
        let mem = InMemoryTileMemory::new();
        let ids = vec![Uuid::new_v4()];
        let retrieved = mem.retrieve(&ids).unwrap();
        assert!(retrieved.is_empty());
    }

    #[test]
    fn search_by_content() {
        let mem = InMemoryTileMemory::new();
        let source = Uuid::new_v4();
        let tiles = vec![
            Tile::new(TileKind::Text, b"hello world".to_vec(), source, 0),
            Tile::new(TileKind::Text, b"foo bar".to_vec(), source, 1),
            Tile::new(TileKind::Text, b"hello foo".to_vec(), source, 2),
        ];
        mem.store(&tiles).unwrap();

        let results = mem.search("hello", None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_by_kind() {
        let mem = InMemoryTileMemory::new();
        let source = Uuid::new_v4();
        let tiles = vec![
            Tile::new(TileKind::Text, b"hello".to_vec(), source, 0),
            Tile::new(TileKind::Binary, b"hello".to_vec(), source, 1),
        ];
        mem.store(&tiles).unwrap();

        let results = mem
            .search("hello", Some(TileKind::Text), 10)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, TileKind::Text);
    }

    #[test]
    fn search_by_meta() {
        let mem = InMemoryTileMemory::new();
        let source = Uuid::new_v4();
        let tile = Tile::new(TileKind::Text, b"data".to_vec(), source, 0)
            .with_meta("author", "alice");
        mem.store(&[tile]).unwrap();

        let results = mem.search("alice", None, 10).unwrap();
        assert_eq!(results.len(), 1);
    }
}
