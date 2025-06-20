use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_ITEMS: usize = 2000;
pub const MAX_AI_ITEMS: usize = 200;
pub const MAX_GROUPS: usize = 40;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Item {
    pub id: u64,
    pub path: PathBuf,
    pub name: String,
    pub extension: String,
    pub kind: String,
    pub category: String,
}

impl Item {
    pub fn from_path(id: u64, path: PathBuf, directory: bool) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let category = if directory {
            "Folders"
        } else {
            category_for(&extension)
        }
        .to_owned();
        let kind = if directory {
            "folder"
        } else if ["lnk", "url"].contains(&extension.as_str()) {
            "shortcut"
        } else {
            "file"
        }
        .to_owned();
        Self {
            id,
            path,
            name,
            extension,
            kind,
            category,
        }
    }
}

pub fn category_for(extension: &str) -> &'static str {
    match extension {
        "lnk" | "url" | "exe" | "msi" => "Applications",
        "doc" | "docx" | "pdf" | "txt" | "md" | "rtf" | "odt" => "Documents",
        "xls" | "xlsx" | "csv" | "tsv" | "ods" => "Data",
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "ico" => "Images",
        "mp4" | "mkv" | "mov" | "avi" | "webm" => "Video",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "Audio",
        "zip" | "7z" | "rar" | "tar" | "gz" => "Archives",
        "rs" | "py" | "js" | "ts" | "html" | "css" | "json" | "toml" | "cpp" | "h" => "Code",
        _ => "Other",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Placement {
    pub monitor: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            monitor: String::new(),
            x: 48,
            y: 64,
            width: 360,
            height: 340,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub items: Vec<u64>,
    pub placement: Placement,
    pub collapsed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub root: PathBuf,
    pub startup_update_check: bool,
    pub hide_on_fullscreen: bool,
    pub opacity: u8,
    pub accent: u32,
    pub ai_base_url: String,
    pub ai_model: String,
    pub protected_api_key: Vec<u8>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            startup_update_check: true,
            hide_on_fullscreen: true,
            opacity: 245,
            accent: 0x00b89a57,
            ai_base_url: "http://localhost:8080/v1".into(),
            ai_model: "universal".into(),
            protected_api_key: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub schema: u32,
    pub next_id: u64,
    pub items: Vec<Item>,
    pub groups: Vec<Group>,
    pub settings: Settings,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            schema: 1,
            next_id: 2,
            items: vec![],
            groups: vec![Group {
                id: 1,
                name: "Inbox".into(),
                items: vec![],
                placement: Placement::default(),
                collapsed: false,
            }],
            settings: Settings::default(),
        }
    }
}
impl Layout {
    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn add_group(&mut self, name: &str) -> Option<u64> {
        if self.groups.len() >= MAX_GROUPS {
            return None;
        }
        let id = self.allocate_id();
        let mut placement = Placement::default();
        placement.x += (self.groups.len() as i32 % 4) * 40;
        placement.y += (self.groups.len() as i32 % 4) * 40;
        self.groups.push(Group {
            id,
            name: name.chars().take(60).collect(),
            items: vec![],
            placement,
            collapsed: false,
        });
        Some(id)
    }
    pub fn import(&mut self, path: PathBuf, directory: bool, group: u64) -> bool {
        if self.items.len() >= MAX_ITEMS || !self.groups.iter().any(|g| g.id == group) {
            return false;
        }
        let id = if let Some(item) = self.items.iter().find(|i| same_path(&i.path, &path)) {
            item.id
        } else {
            let id = self.allocate_id();
            self.items.push(Item::from_path(id, path, directory));
            id
        };
        self.assign(id, group);
        true
    }
    pub fn assign(&mut self, item: u64, group: u64) {
        if !self.items.iter().any(|i| i.id == item) || !self.groups.iter().any(|g| g.id == group) {
            return;
        }
        for g in &mut self.groups {
            g.items.retain(|id| *id != item);
            if g.id == group {
                g.items.push(item);
            }
        }
    }
    pub fn refresh_reference(
        &mut self,
        path: PathBuf,
        directory: bool,
        fallback_group: u64,
    ) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| same_path(&i.path, &path)) {
            *item = Item::from_path(item.id, path, directory);
            true
        } else {
            self.import(path, directory, fallback_group)
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::ensure;
        use std::collections::HashSet;
        ensure!(self.schema == 1, "Unsupported layout schema");
        ensure!(
            !self.groups.is_empty()
                && self.groups.len() <= MAX_GROUPS
                && self.items.len() <= MAX_ITEMS,
            "Layout size outside limits"
        );
        let mut ids = HashSet::new();
        for i in &self.items {
            ensure!(
                ids.insert(i.id) && i.id < self.next_id && i.path.is_absolute(),
                "Invalid item ID or path"
            );
        }
        for g in &self.groups {
            ensure!(
                ids.insert(g.id)
                    && g.id < self.next_id
                    && !g.name.trim().is_empty()
                    && g.name.len() <= 240,
                "Invalid group ID or name"
            );
            ensure!(
                g.placement.x.unsigned_abs() <= 100_000 && g.placement.y.unsigned_abs() <= 100_000,
                "Group placement outside limits"
            );
        }
        let mut assigned = HashSet::new();
        for g in &self.groups {
            for id in &g.items {
                ensure!(
                    self.items.iter().any(|i| i.id == *id) && assigned.insert(*id),
                    "Invalid or duplicate item assignment"
                );
            }
        }
        ensure!(
            assigned.len() == self.items.len(),
            "Unassigned items in layout"
        );
        ensure!(
            self.next_id < u64::MAX - 10_000,
            "Layout ID space exhausted"
        );
        ensure!(
            self.settings.opacity >= 150
                && self.settings.ai_base_url.len() <= 4096
                && self.settings.ai_model.len() < 256
                && self.settings.protected_api_key.len() <= 64 * 1024,
            "Settings outside limits"
        );
        Ok(())
    }
}
pub fn same_path(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refreshing_a_folder_keeps_manual_group_assignments() {
        let root = std::env::current_dir().unwrap();
        let mut layout = Layout::default();
        let work = layout.add_group("Work").unwrap();
        let path = root.join("notes.txt");
        layout.import(path.clone(), false, work);
        let id = layout.items[0].id;
        assert!(layout.refresh_reference(path, false, 1));
        assert!(
            layout
                .groups
                .iter()
                .find(|g| g.id == work)
                .unwrap()
                .items
                .contains(&id)
        );
        assert!(layout.groups[0].items.is_empty());
        layout.refresh_reference(root.join("new.txt"), false, 1);
        assert_eq!(layout.groups[0].items.len(), 1);
        layout.validate().unwrap();
    }
}
