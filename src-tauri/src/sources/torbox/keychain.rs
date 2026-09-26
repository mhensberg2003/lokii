//! The TorBox API key in the OS keychain (macOS Keychain, Windows Credential Manager).

const SERVICE: &str = "app.lokii.desktop";
const ACCOUNT: &str = "torbox-api-key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("cannot open the keychain: {e}"))
}

pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("cannot read the TorBox API key from the keychain: {e}")),
    }
}

pub fn save(key: &str) -> Result<(), String> {
    entry()?.set_password(key).map_err(|e| format!("cannot save the TorBox API key in the keychain: {e}"))
}

pub fn delete() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("cannot remove the TorBox API key from the keychain: {e}")),
    }
}
