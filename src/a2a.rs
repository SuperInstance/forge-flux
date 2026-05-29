use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A tick emitted by the forge pipeline for A2A integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeTick {
    pub pipeline_id: Uuid,
    pub stage: String,
    pub tiles_in: u64,
    pub tiles_out: u64,
    pub cr: f64,
    pub timestamp: i64,
}

impl ForgeTick {
    pub fn new(pipeline_id: Uuid, stage: impl Into<String>, tiles_in: u64, tiles_out: u64, cr: f64) -> Self {
        Self {
            pipeline_id,
            stage: stage.into(),
            tiles_in,
            tiles_out,
            cr,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// Collect ticks from a pipeline run.
#[derive(Debug, Default)]
pub struct TickCollector {
    pub ticks: Vec<ForgeTick>,
}

impl TickCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, tick: ForgeTick) {
        self.ticks.push(tick);
    }

    /// Calculate overall conservation ratio from all ticks.
    pub fn overall_cr(&self) -> f64 {
        if self.ticks.is_empty() {
            return 1.0;
        }
        self.ticks.iter().map(|t| t.cr).sum::<f64>() / self.ticks.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_creation() {
        let id = Uuid::new_v4();
        let tick = ForgeTick::new(id, "decompose", 1, 5, 1.0);
        assert_eq!(tick.pipeline_id, id);
        assert_eq!(tick.stage, "decompose");
        assert_eq!(tick.tiles_in, 1);
        assert_eq!(tick.tiles_out, 5);
    }

    #[test]
    fn tick_serialization() {
        let id = Uuid::new_v4();
        let tick = ForgeTick::new(id, "transform", 5, 5, 0.95);
        let json = tick.to_json();
        let back = ForgeTick::from_json(&json).unwrap();
        assert_eq!(back.pipeline_id, id);
        assert_eq!(back.stage, "transform");
        assert!((back.cr - 0.95).abs() < 1e-10);
    }

    #[test]
    fn tick_collector() {
        let id = Uuid::new_v4();
        let mut collector = TickCollector::new();
        collector.record(ForgeTick::new(id, "decompose", 1, 3, 1.0));
        collector.record(ForgeTick::new(id, "transform", 3, 3, 0.9));
        collector.record(ForgeTick::new(id, "assemble", 3, 1, 0.8));
        assert_eq!(collector.ticks.len(), 3);
        let cr = collector.overall_cr();
        assert!((cr - 0.9).abs() < 0.01);
    }
}
