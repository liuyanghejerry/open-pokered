use dotzuki_engine::save::{SaveError, SaveStorage};
use std::path::PathBuf;

const SAVE_FILE_NAME: &str = "save.dat";

pub struct FileSystemStorage {
    dir: PathBuf,
}

impl FileSystemStorage {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn slot_path(&self, slot: usize) -> PathBuf {
        if slot == 0 {
            self.dir.join(SAVE_FILE_NAME)
        } else {
            self.dir.join(format!("save_{}.dat", slot))
        }
    }
}

impl SaveStorage for FileSystemStorage {
    fn write(&self, slot: usize, data: &[u8]) -> Result<(), SaveError> {
        let path = self.slot_path(slot);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SaveError::IoError(e.to_string()))?;
        }
        std::fs::write(&path, data).map_err(|e| SaveError::IoError(e.to_string()))
    }

    fn read(&self, slot: usize) -> Result<Vec<u8>, SaveError> {
        let path = self.slot_path(slot);
        if !path.exists() {
            return Err(SaveError::SlotEmpty);
        }
        std::fs::read(&path).map_err(|e| SaveError::IoError(e.to_string()))
    }

    fn slot_exists(&self, slot: usize) -> bool {
        self.slot_path(slot).exists()
    }

    fn delete_slot(&self, slot: usize) -> Result<(), SaveError> {
        let path = self.slot_path(slot);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| SaveError::IoError(e.to_string()))
        } else {
            Ok(())
        }
    }
}
