//! Two most recently tested plans, stored locally without test results.
use crate::sequence_protocol::{self as plan, Step};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 8 + 2 * (8 + plan::MAX_STEPS * 12);

#[derive(Default)]
pub struct History {
    pub entries: Vec<Vec<Step>>,
}

fn path() -> io::Result<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"));
    base.map(|base| base.join("KeyCrash/sequence-history.bin"))
        .ok_or_else(|| io::Error::other("User data directory is unavailable"))
}

impl History {
    fn update(change: impl FnOnce(&mut Self)) -> Result<Self, String> {
        let path = path().map_err(|e| e.to_string())?;
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        // A busy writer is an explicit save error; never wait on the GUI thread.
        lock.try_lock().map_err(|e| e.to_string())?;
        let mut current = Self::read_from(&path)?;
        change(&mut current);
        current.write_to(&path)?;
        Ok(current)
    }

    pub fn record(steps: &[Step]) -> Result<Self, String> {
        Self::update(|history| history.remember(steps))
    }

    pub fn delete(steps: &[Step]) -> Result<Self, String> {
        Self::update(|history| history.entries.retain(|entry| entry != steps))
    }

    pub fn load() -> Result<Self, String> {
        Self::read_from(&path().map_err(|e| e.to_string())?)
    }

    pub fn read_from(path: &Path) -> Result<Self, String> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES || bytes.len() < 8 || &bytes[..4] != b"KSH1" {
            return Err("Invalid sequence history".into());
        }
        let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if count > 2 {
            return Err("Invalid history length".into());
        }
        let mut input = Cursor::new(&bytes[8..]);
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(plan::read_plan(&mut input)?);
        }
        if input.position() as usize != bytes.len() - 8 {
            return Err("Unexpected history data".into());
        }
        Ok(Self { entries })
    }

    pub fn remember(&mut self, steps: &[Step]) {
        self.entries.retain(|entry| entry != steps);
        self.entries.insert(0, steps.to_vec());
        self.entries.truncate(2);
    }

    pub fn write_to(&self, path: &Path) -> Result<(), String> {
        if self.entries.len() > 2 {
            return Err("Invalid history length".into());
        }
        let mut bytes = b"KSH1".to_vec();
        bytes.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for entry in &self.entries {
            plan::write_plan(entry, &mut bytes)?;
        }
        let parent = path.parent().ok_or("History directory is unavailable")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let result = result.and_then(|()| fs::rename(&temporary, path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())
    }
}
