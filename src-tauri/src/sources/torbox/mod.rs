//! TorBox Source: the account connection and the REST client. The API key lives in the
//! OS keychain; this state keeps a copy in memory after the first read.

pub mod client;
mod keychain;

use std::sync::RwLock;

use serde::Serialize;
use tauri::State;

use client::{Account, TorBox};

pub struct TorBoxState {
    http: reqwest::Client,
    /// `None` until the keychain was read once.
    key: RwLock<Option<Option<String>>>,
}

impl Default for TorBoxState {
    fn default() -> Self {
        Self { http: crate::index::http_client(), key: RwLock::new(None) }
    }
}

impl TorBoxState {
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// The API key, or `None` when TorBox is not connected.
    pub fn key(&self) -> Result<Option<String>, String> {
        if let Some(key) = self.key.read().map_err(|_| LOCK_ERROR)?.clone() {
            return Ok(key);
        }
        let key = keychain::load()?;
        *self.key.write().map_err(|_| LOCK_ERROR)? = Some(key.clone());
        Ok(key)
    }

    fn set_key(&self, key: Option<String>) -> Result<(), String> {
        match &key {
            Some(key) => keychain::save(key)?,
            None => keychain::delete()?,
        }
        *self.key.write().map_err(|_| LOCK_ERROR)? = Some(key);
        Ok(())
    }
}

const LOCK_ERROR: &str = "the TorBox state is broken; restart Lokii";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorBoxStatus {
    connected: bool,
    /// The last 4 characters of the API key.
    key_hint: Option<String>,
    account: Option<Account>,
    /// Why the account could not load, while the key is still saved.
    error: Option<String>,
}

#[tauri::command]
pub async fn torbox_status(state: State<'_, TorBoxState>) -> Result<TorBoxStatus, String> {
    let Some(key) = state.key()? else {
        return Ok(TorBoxStatus { connected: false, key_hint: None, account: None, error: None });
    };
    let result = TorBox { http: &state.http, key: &key }.account().await;
    let (account, error) = match result {
        Ok(account) => (Some(account), None),
        Err(err) => (None, Some(err)),
    };
    Ok(TorBoxStatus { connected: true, key_hint: Some(key_hint(&key)), account, error })
}

/// Checks the key with TorBox, then saves it in the keychain.
#[tauri::command]
pub async fn torbox_connect(state: State<'_, TorBoxState>, api_key: String) -> Result<TorBoxStatus, String> {
    let key = api_key.trim().to_string();
    if key.is_empty() {
        return Err("Paste your TorBox API key.".to_string());
    }
    let account = TorBox { http: &state.http, key: &key }.account().await?;
    state.set_key(Some(key.clone()))?;
    Ok(TorBoxStatus { connected: true, key_hint: Some(key_hint(&key)), account: Some(account), error: None })
}

#[tauri::command]
pub fn torbox_disconnect(state: State<'_, TorBoxState>) -> Result<(), String> {
    state.set_key(None)
}

fn key_hint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_hint_is_the_last_four_characters() {
        assert_eq!(key_hint("abcdef123456"), "3456");
        assert_eq!(key_hint("ab"), "ab");
    }
}
