use crate::model::PetSettings;
use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            path: app_data_dir.join("settings.json"),
        }
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> io::Result<PetSettings> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(PetSettings::default()),
            Err(error) => return Err(error),
        };

        let mut settings = serde_json::from_slice::<PetSettings>(&bytes).unwrap_or_default();
        settings.normalize();
        Ok(settings)
    }

    pub fn save(&self, settings: &PetSettings) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_vec_pretty(settings)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, contents)?;
        replace_file(&temporary, &self.path)
    }
}

fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        #[cfg(target_os = "windows")]
        Err(_) if destination.exists() => {
            // std::fs::rename cannot replace an existing file on Windows. The old
            // settings remain intact until the fully-written temporary exists.
            let backup = destination.with_extension("json.bak");
            let _ = fs::remove_file(&backup);
            fs::rename(destination, &backup)?;
            match fs::rename(source, destination) {
                Ok(()) => {
                    let _ = fs::remove_file(backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(backup, destination);
                    Err(error)
                }
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PetScale, SETTINGS_SCHEMA_VERSION};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_store(name: &str) -> SettingsStore {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        SettingsStore::at(std::env::temp_dir().join(format!(
            "yunweishou-{name}-{}-{unique}.json",
            std::process::id()
        )))
    }

    #[test]
    fn missing_file_uses_defaults() {
        let store = temporary_store("missing");
        assert_eq!(store.load().unwrap(), PetSettings::default());
    }

    #[test]
    fn corrupt_file_falls_back_without_panicking() {
        let store = temporary_store("corrupt");
        fs::write(&store.path, b"not json").unwrap();
        assert_eq!(store.load().unwrap(), PetSettings::default());
        let _ = fs::remove_file(store.path);
    }

    #[test]
    fn round_trip_and_migration_normalize_values() {
        let store = temporary_store("round-trip");
        fs::write(
            &store.path,
            br#"{"schemaVersion":0,"scale":"large","normalizedX":2.5,"tutorialStep":9}"#,
        )
        .unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(loaded.scale, PetScale::Large);
        assert_eq!(loaded.normalized_x, 1.0);
        assert_eq!(loaded.tutorial_step, 3);
        store.save(&loaded).unwrap();
        assert_eq!(store.load().unwrap(), loaded);
        let _ = fs::remove_file(store.path);
    }
}
