//! Local Torrent settings in the `meta` table: the listening port and the folder for
//! torrent data. The engine reads them when it starts, so a change applies after a restart.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::LocalTorrentState;
use crate::store::{Store, StoreState};

const PORT_KEY: &str = "local_torrent_port";
const FOLDER_KEY: &str = "local_torrent_folder";
/// Lokii only writes and deletes inside this subfolder of the chosen folder.
const DATA_SUBFOLDER: &str = "lokii-streams";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalSettings {
    /// `None` lets the OS choose a free port.
    pub port: Option<u16>,
    /// `None` uses the app's cache folder.
    pub folder: Option<PathBuf>,
}

impl LocalSettings {
    pub async fn load(store: &Store) -> Result<Self, String> {
        let port = store.meta(PORT_KEY).await?.and_then(|v| v.parse().ok());
        let folder = store.meta(FOLDER_KEY).await?.map(PathBuf::from);
        Ok(Self { port, folder })
    }

    async fn save(&self, store: &Store) -> Result<(), String> {
        store.set_meta(PORT_KEY, self.port.map(|p| p.to_string())).await?;
        store.set_meta(FOLDER_KEY, self.folder.as_ref().map(|f| f.to_string_lossy().into_owned())).await
    }

    /// The folder that holds torrent data while Streams play.
    pub fn data_dir(&self, app: &AppHandle) -> Result<PathBuf, String> {
        let base = match &self.folder {
            Some(folder) => folder.clone(),
            None => app.path().app_cache_dir().map_err(|e| format!("cannot find the app cache folder: {e}"))?,
        };
        Ok(base.join(DATA_SUBFOLDER))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSettingsView {
    port: Option<u16>,
    folder: Option<String>,
    /// Where the data goes with these settings.
    data_dir: String,
    /// The running engine uses other settings; they apply after a restart.
    restart_needed: bool,
}

#[tauri::command]
pub async fn local_settings(
    app: AppHandle,
    store: State<'_, StoreState>,
    state: State<'_, LocalTorrentState>,
) -> Result<LocalSettingsView, String> {
    let settings = LocalSettings::load(store.get(&app).await?).await?;
    view(&app, &state, settings)
}

#[tauri::command]
pub async fn local_set_settings(
    app: AppHandle,
    store: State<'_, StoreState>,
    state: State<'_, LocalTorrentState>,
    port: Option<u16>,
    folder: Option<String>,
) -> Result<LocalSettingsView, String> {
    let folder = folder.filter(|f| !f.trim().is_empty()).map(PathBuf::from);
    if let Some(folder) = &folder {
        check_folder(folder)?;
    }
    if port.is_some_and(|p| p < 1024) {
        return Err("Use a port from 1024 to 65535.".to_string());
    }
    let settings = LocalSettings { port, folder };
    settings.save(store.get(&app).await?).await?;
    view(&app, &state, settings)
}

fn view(app: &AppHandle, state: &LocalTorrentState, settings: LocalSettings) -> Result<LocalSettingsView, String> {
    let restart_needed = state.started_with().is_some_and(|running| *running != settings);
    Ok(LocalSettingsView {
        port: settings.port,
        folder: settings.folder.as_ref().map(|f| f.to_string_lossy().into_owned()),
        data_dir: settings.data_dir(app)?.to_string_lossy().into_owned(),
        restart_needed,
    })
}

fn check_folder(folder: &Path) -> Result<(), String> {
    if !folder.is_absolute() || !folder.is_dir() {
        return Err(format!("{} is not a folder.", folder.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn settings_round_trip_through_the_store() {
        let store = Store::in_memory().unwrap();
        assert_eq!(LocalSettings::load(&store).await.unwrap(), LocalSettings::default());
        let settings = LocalSettings { port: Some(51413), folder: Some(PathBuf::from("/tmp")) };
        settings.save(&store).await.unwrap();
        assert_eq!(LocalSettings::load(&store).await.unwrap(), settings);
        LocalSettings::default().save(&store).await.unwrap();
        assert_eq!(LocalSettings::load(&store).await.unwrap(), LocalSettings::default());
    }

    #[test]
    fn only_an_existing_absolute_folder_is_accepted() {
        assert!(check_folder(&std::env::temp_dir()).is_ok());
        assert!(check_folder(Path::new("relative/folder")).is_err());
        assert!(check_folder(Path::new("/no/such/folder/for/lokii")).is_err());
    }
}
