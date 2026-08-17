use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentDevice {
    pub id: String,
    pub name: String,
    pub os: String,
    pub favorite: bool,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentsStore {
    pub items: Vec<RecentDevice>,
}

impl RecentsStore {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = recents_path(dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(recents_path(dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn touch(&mut self, id: String, name: String, os: String) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.name = name;
            item.os = os;
            item.last_seen = Utc::now();
        } else {
            self.items.push(RecentDevice {
                id,
                name,
                os,
                favorite: false,
                last_seen: Utc::now(),
            });
        }
        self.items.sort_by(|a, b| {
            b.favorite
                .cmp(&a.favorite)
                .then(b.last_seen.cmp(&a.last_seen))
        });
        self.items.truncate(20);
    }

    pub fn toggle_favorite(&mut self, id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.favorite = !item.favorite;
        }
    }
}

fn recents_path(dir: &Path) -> PathBuf {
    dir.join("recents.json")
}
