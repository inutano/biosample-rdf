use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub source_file: String,
    pub source_size: u64,
    pub source_md5: String,
    pub chunks_completed: u32,
    pub records_processed: u64,
    pub records_skipped: u64,
    pub started_at: String,
}

impl Progress {
    pub fn new(source_file: &str, source_size: u64, source_md5: &str) -> Self {
        Self {
            source_file: source_file.to_string(),
            source_size,
            source_md5: source_md5.to_string(),
            chunks_completed: 0,
            records_processed: 0,
            records_skipped: 0,
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = fs::read_to_string(path)?;
        let progress: Self = serde_json::from_str(&data)?;
        Ok(progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_progress_save_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("progress.json");

        let mut p = Progress::new("input.xml.gz", 1_000_000, "abc123def456");
        p.chunks_completed = 3;
        p.records_processed = 300_000;
        p.records_skipped = 5;

        p.save(&path).unwrap();

        let loaded = Progress::load(&path).unwrap();
        assert_eq!(loaded.source_file, "input.xml.gz");
        assert_eq!(loaded.source_size, 1_000_000);
        assert_eq!(loaded.source_md5, "abc123def456");
        assert_eq!(loaded.chunks_completed, 3);
        assert_eq!(loaded.records_processed, 300_000);
        assert_eq!(loaded.records_skipped, 5);
        assert!(!loaded.started_at.is_empty());
    }

    #[test]
    fn test_progress_new_defaults() {
        let p = Progress::new("test.xml", 42, "deadbeef");
        assert_eq!(p.source_file, "test.xml");
        assert_eq!(p.source_size, 42);
        assert_eq!(p.source_md5, "deadbeef");
        assert_eq!(p.chunks_completed, 0);
        assert_eq!(p.records_processed, 0);
        assert_eq!(p.records_skipped, 0);
        assert!(!p.started_at.is_empty());
    }

    #[test]
    fn test_progress_save_creates_valid_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("progress.json");

        let p = Progress::new("source.xml", 999, "ff00ff");
        p.save(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(val["source_file"], "source.xml");
        assert_eq!(val["source_size"], 999);
        assert_eq!(val["source_md5"], "ff00ff");
    }
}
