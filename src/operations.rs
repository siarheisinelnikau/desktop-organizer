//! Deliberate, journaled, same-volume moves. No overwrites or recursive folder moves.
use crate::{grouping::{self, Suggestion}, model::Item, persistence};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::{self, File, OpenOptions}, io::{Read, Seek, SeekFrom}, path::{Component, Path, PathBuf}};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprint { pub len: u64, pub sha256: String }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Move { pub item_id: u64, pub source: PathBuf, pub destination: PathBuf, pub fingerprint: Fingerprint, pub state: MoveState }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MoveState { Pending, Moving, Done, Undoing, Undone }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan { pub root: PathBuf, pub moves: Vec<Move>, pub created_directories: Vec<PathBuf>, pub completed: bool }

pub fn safe_name(name: &str) -> Result<()> {
    ensure!(!name.is_empty() && name.chars().count() <= 60 && name.trim() == name && !name.ends_with('.'), "Folder name cannot be empty, padded, too long or end with a dot");
    ensure!(!name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)) && name != "." && name != "..", "Unsafe folder name: {name}");
    let stem = name.split('.').next().unwrap().to_uppercase();
    ensure!(!["CON", "PRN", "AUX", "NUL", "CLOCK$"].contains(&stem.as_str()) && !(stem.starts_with("COM") || stem.starts_with("LPT")) .then(|| stem[3..].parse::<u8>().ok()).flatten().is_some_and(|n| (1..=9).contains(&n)), "Reserved Windows folder name");
    Ok(())
}

fn reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)] { use std::os::windows::fs::MetadataExt; meta.file_attributes() & 0x400 != 0 }
    #[cfg(not(windows))] { meta.file_type().is_symlink() }
}

pub fn checked_path(path: &Path, must_exist: bool) -> Result<PathBuf> {
    ensure!(path.is_absolute(), "Use an absolute local path");
    ensure!(!path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)), "Path traversal is forbidden");
    #[cfg(windows)] {
        let text = path.to_string_lossy();
        ensure!(!text.starts_with("\\\\") && !text[2..].contains(':'), "UNC, device paths and alternate streams are not supported");
    }
    let mut walked = PathBuf::new();
    for component in path.components() {
        walked.push(component);
        match fs::symlink_metadata(&walked) {
            Ok(meta) => ensure!(!reparse(&meta), "Reparse points and symbolic links are forbidden: {}", walked.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !must_exist => {},
            Err(e) => return Err(e).with_context(|| format!("Cannot inspect {}", walked.display())),
        }
    }
    Ok(path.to_path_buf())
}

pub fn fingerprint(file: &mut File) -> Result<Fingerprint> {
    file.seek(SeekFrom::Start(0))?;
    let len = file.metadata()?.len();
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop { let n = file.read(&mut buffer)?; if n == 0 { break; } hash.update(&buffer[..n]); }
    ensure!(file.metadata()?.len() == len, "File changed during verification");
    Ok(Fingerprint { len, sha256: format!("{:x}", hash.finalize()) })
}

pub fn preview(root: &Path, items: &[Item], groups: &[Suggestion]) -> Result<Plan> {
    grouping::validate(groups, items)?;
    let root = checked_path(root, true)?;
    ensure!(root.is_dir(), "Selected root is not a directory");
    let mut moves = vec![];
    let mut reserved = HashSet::new();
    for group in groups {
        safe_name(&group.name)?;
        let directory = root.join(&group.name);
        checked_path(&directory, false)?;
        ensure!(!directory.exists() || directory.is_dir(), "Group path is an existing file");
        for id in &group.item_ids {
            let item = items.iter().find(|i| i.id == *id).unwrap();
            let source = checked_path(&item.path, true)?;
            ensure!(source.parent() == Some(root.as_path()), "Only direct children of the selected root can be moved");
            ensure!(source.is_file(), "Folders can be grouped visually but cannot be moved");
            let name = source.file_name().context("Source has no file name")?;
            let mut destination = directory.join(name);
            let mut count = 1;
            while destination.exists() || reserved.contains(&destination.to_string_lossy().to_lowercase()) {
                let stem = source.file_stem().unwrap_or_default().to_string_lossy();
                let ext = source.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                destination = directory.join(format!("{stem} ({count}){ext}"));
                count += 1;
                ensure!(count <= 10_000, "Too many filename collisions");
            }
            checked_path(&destination, false)?;
            reserved.insert(destination.to_string_lossy().to_lowercase());
            let fingerprint = fingerprint(&mut File::open(&source)?)?;
            moves.push(Move { item_id: *id, source, destination, fingerprint, state: MoveState::Pending });
        }
    }
    Ok(Plan { root, moves, created_directories: vec![], completed: false })
}

impl Plan {
    pub fn validate(&self) -> Result<()> {
        checked_path(&self.root, true)?;
        ensure!(!self.moves.is_empty() && self.moves.len() <= crate::model::MAX_ITEMS, "Invalid move count");
        let mut sources = HashSet::new();
        let mut targets = HashSet::new();
        for m in &self.moves {
            ensure!(m.source.parent() == Some(self.root.as_path()), "Source escaped selected root");
            let parent = m.destination.parent().context("Invalid destination")?;
            ensure!(parent.parent() == Some(self.root.as_path()), "Destination escaped selected root");
            safe_name(&parent.file_name().context("Invalid folder")?.to_string_lossy())?;
            ensure!(m.destination.file_name().is_some() && m.destination != m.source, "Invalid destination");
            checked_path(&m.source, false)?;
            checked_path(&m.destination, false)?;
            ensure!(sources.insert(m.source.to_string_lossy().to_lowercase()) && targets.insert(m.destination.to_string_lossy().to_lowercase()), "Duplicate move");
        }
        for d in &self.created_directories { ensure!(d.parent() == Some(self.root.as_path()), "Journal directory escaped root"); checked_path(d, false)?; }
        Ok(())
    }

    pub fn apply(&mut self, journal: &Path) -> Result<()> {
        self.validate()?;
        ensure!(self.moves.iter().all(|m| m.state == MoveState::Pending), "Plan already started. Recover or undo its journal.");
        persistence::save(journal, self)?;
        let _root_guard = directory_guards(&self.root)?;
        for index in 0..self.moves.len() {
            let directory = self.moves[index].destination.parent().unwrap().to_path_buf();
            checked_path(&directory, false)?;
            if !directory.exists() {
                // Record intent first: recovery never deletes a nonempty directory.
                self.created_directories.push(directory.clone());
                persistence::save(journal, self)?;
                fs::create_dir(&directory).context("Cannot create group folder")?;
            }
            let _target_guard = directory_guards(&directory)?;
            self.moves[index].state = MoveState::Moving;
            persistence::save(journal, self)?;
            move_verified(&self.moves[index].source, &self.moves[index].destination, &self.moves[index].fingerprint)?;
            self.moves[index].state = MoveState::Done;
            persistence::save(journal, self)?;
        }
        self.completed = true;
        persistence::save(journal, self)
    }

    pub fn recover(&mut self) -> Result<()> {
        self.validate()?;
        for m in &mut self.moves {
            if matches!(m.state, MoveState::Moving | MoveState::Undoing) {
                let source = m.source.exists();
                let dest = m.destination.exists();
                ensure!(source != dest, "Ambiguous recovery; both or neither path exists. Resolve manually using the journal.");
                let actual = if dest { &m.destination } else { &m.source };
                ensure!(fingerprint(&mut File::open(actual)?)? == m.fingerprint, "File changed since operation. Resolve manually using the journal.");
                m.state = if dest { MoveState::Done } else if m.state == MoveState::Undoing { MoveState::Undone } else { MoveState::Pending };
            }
        }
        Ok(())
    }

    pub fn undo(&mut self, journal: &Path) -> Result<()> {
        self.recover()?;
        let _root_guard = directory_guards(&self.root)?;
        for index in (0..self.moves.len()).rev() {
            if self.moves[index].state != MoveState::Done { continue; }
            let _target_guard = directory_guards(self.moves[index].destination.parent().unwrap())?;
            self.moves[index].state = MoveState::Undoing;
            persistence::save(journal, self)?;
            move_verified(&self.moves[index].destination, &self.moves[index].source, &self.moves[index].fingerprint)?;
            self.moves[index].state = MoveState::Undone;
            persistence::save(journal, self)?;
        }
        for directory in &self.created_directories { if directory.exists() { checked_path(directory, true)?; let _ = fs::remove_dir(directory); } }
        self.completed = false;
        persistence::save(journal, self)
    }
}

fn directory_guards(path: &Path) -> Result<Vec<File>> {
    checked_path(path, true)?;
    let mut guards = vec![];
    let mut walked = PathBuf::new();
    for part in path.components() {
        walked.push(part);
        if matches!(part, Component::Prefix(_)) { continue; }
        if !walked.is_dir() { continue; }
        #[cfg(windows)] {
            use std::os::windows::fs::OpenOptionsExt;
            // Excluding FILE_SHARE_DELETE prevents directory/junction substitution until the move finishes.
            guards.push(OpenOptions::new().access_mode(0x80).share_mode(3).custom_flags(0x02200000).open(&walked).with_context(|| format!("Cannot guard directory {}", walked.display()))?);
        }
        #[cfg(not(windows))] guards.push(File::open(&walked)?);
    }
    checked_path(path, true)?;
    Ok(guards)
}

fn move_verified(source: &Path, destination: &Path, expected: &Fingerprint) -> Result<()> {
    checked_path(source, true)?;
    checked_path(destination, false)?;
    ensure!(!destination.exists(), "Destination now exists; no files were overwritten");
    #[cfg(windows)] {
        use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{FILE_RENAME_INFO, FileRenameInfo, SetFileInformationByHandle};
        // DELETE access and no write/delete sharing keep the verified source stable.
        let mut file = OpenOptions::new().access_mode(0x80000000 | 0x00010000).share_mode(1).custom_flags(0x00200000).open(source).context("Cannot lock source for atomic rename")?;
        ensure!(file.metadata()?.is_file() && !reparse(&file.metadata()?), "Source is not a regular file");
        ensure!(&fingerprint(&mut file)? == expected, "File changed after preview; generate a new plan");
        let name: Vec<u16> = destination.as_os_str().encode_wide().collect();
        let size = std::mem::size_of::<FILE_RENAME_INFO>() + name.len() * 2;
        let mut buffer = vec![0u64; size.div_ceil(8)];
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        unsafe {
            (*info).RootDirectory = std::ptr::null_mut();
            (*info).FileNameLength = (name.len() * 2) as u32;
            std::ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
            // Zeroed ReplaceIfExists: atomic rename must fail on collision.
            if SetFileInformationByHandle(file.as_raw_handle(), FileRenameInfo, info.cast(), size as u32) == 0 {
                return Err(std::io::Error::last_os_error()).context("Atomic rename failed; source retained");
            }
        }
    }
    #[cfg(not(windows))] {
        ensure!(&fingerprint(&mut File::open(source)?)? == expected, "File changed after preview");
        fs::hard_link(source, destination).context("Same-volume move required")?;
        if let Err(e) = fs::remove_file(source) { let _ = fs::remove_file(destination); return Err(e.into()); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Vec<Item>, Vec<Suggestion>) {
        fs::create_dir_all(".test-output").unwrap();
        let temp = tempfile::tempdir_in(std::env::current_dir().unwrap().join(".test-output")).unwrap();
        // Canonical path normalizes the temporary directory on each platform.
        let root = temp.path().to_path_buf();
        fs::write(root.join("a.txt"), "original").unwrap();
        let items = vec![Item::from_path(1, root.join("a.txt"), false)];
        let groups = vec![Suggestion { name: "Documents".into(), item_ids: vec![1] }];
        (temp, items, groups)
    }
    #[test]
    fn collisions_move_and_undo_preserve_both_files() {
        let (temp, items, groups) = fixture();
        fs::create_dir(temp.path().join("Documents")).unwrap();
        fs::write(temp.path().join("Documents/a.txt"), "existing").unwrap();
        let mut plan = preview(temp.path(), &items, &groups).unwrap();
        assert!(plan.moves[0].destination.ends_with("a (1).txt"));
        let journal = temp.path().join("journal.json");
        plan.apply(&journal).unwrap();
        assert!(!items[0].path.exists());
        plan.undo(&journal).unwrap();
        assert_eq!(fs::read_to_string(&items[0].path).unwrap(), "original");
        assert_eq!(fs::read_to_string(temp.path().join("Documents/a.txt")).unwrap(), "existing");
    }
    #[test]
    fn no_overwrite_on_late_collision_or_undo_conflict() {
        let (temp, items, groups) = fixture();
        let mut plan = preview(temp.path(), &items, &groups).unwrap();
        let journal = temp.path().join("journal.json");
        plan.apply(&journal).unwrap();
        fs::write(&items[0].path, "new").unwrap();
        assert!(plan.undo(&journal).is_err());
        assert_eq!(fs::read_to_string(&items[0].path).unwrap(), "new");
        assert!(plan.moves[0].destination.exists());
    }
    #[test]
    fn rejects_traversal_reserved_names_and_outside_sources() {
        let (temp, items, mut groups) = fixture();
        for name in ["../escape", "CON", "LPT1", " x", "x.", "a/b", "a:b", ".."] { groups[0].name = name.into(); assert!(preview(temp.path(), &items, &groups).is_err(), "{name}"); }
        groups[0].name = "Docs".into();
        let outside = tempfile::tempdir_in(std::env::current_dir().unwrap().join(".test-output")).unwrap();
        assert!(preview(outside.path(), &items, &groups).is_err());
    }
    #[test]
    fn changed_content_is_never_moved_or_undone() {
        let (temp, items, groups) = fixture();
        let mut plan = preview(temp.path(), &items, &groups).unwrap();
        fs::write(&items[0].path, "tampered").unwrap();
        assert!(plan.apply(&temp.path().join("journal.json")).is_err());
        assert!(items[0].path.exists());
    }
    #[test]
    fn recovery_resolves_interrupted_rename() {
        let (temp, items, groups) = fixture();
        let mut plan = preview(temp.path(), &items, &groups).unwrap();
        let journal = temp.path().join("journal.json");
        plan.apply(&journal).unwrap();
        plan.moves[0].state = MoveState::Moving;
        plan.recover().unwrap();
        assert_eq!(plan.moves[0].state, MoveState::Done);
        plan.undo(&journal).unwrap();
        assert!(!temp.path().join("Documents").exists());
    }
}
