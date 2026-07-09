//! OS-backed credential storage for Microsoft account tokens (Windows Credential
//! Manager, macOS Keychain, Linux Secret Service).

use crate::error::{Error, Result};

const SERVICE: &str = "com.ezmapa.launcher";

fn entry(user: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, user).map_err(|e| {
        Error::Other(format!(
            "Could not access secure credential storage: {e}. \
             Try signing in again or check OS keychain permissions."
        ))
    })
}

fn access_user(account_id: &str) -> String {
    format!("account:{account_id}:access")
}

fn refresh_user(account_id: &str) -> String {
    format!("account:{account_id}:refresh")
}

fn map_store_err(account_id: &str, kind: &str, e: keyring::Error) -> Error {
    Error::Other(format!(
        "Could not save {kind} token for account {account_id}: {e}"
    ))
}

fn map_load_err(account_id: &str, kind: &str, e: keyring::Error) -> Error {
    Error::Other(format!(
        "Could not read {kind} token for account {account_id}: {e}. \
         You may need to sign in again."
    ))
}

/// Persist access and optional refresh tokens for a Microsoft account.
pub fn store_tokens(account_id: &str, access: &str, refresh: Option<&str>) -> Result<()> {
    entry(&access_user(account_id))?
        .set_password(access)
        .map_err(|e| map_store_err(account_id, "access", e))?;

    let refresh_entry = entry(&refresh_user(account_id))?;
    match refresh.filter(|r| !r.is_empty()) {
        Some(rt) => refresh_entry
            .set_password(rt)
            .map_err(|e| map_store_err(account_id, "refresh", e))?,
        None => {
            // Drop a stale refresh token when the account no longer has one.
            let _ = refresh_entry.delete_credential();
        }
    }
    Ok(())
}

/// Load tokens from secure storage. Missing entries yield empty / None.
pub fn load_tokens(account_id: &str) -> Result<(String, Option<String>)> {
    let access = match entry(&access_user(account_id))?.get_password() {
        Ok(p) => p,
        Err(keyring::Error::NoEntry) => String::new(),
        Err(e) => return Err(map_load_err(account_id, "access", e)),
    };
    let refresh = match entry(&refresh_user(account_id))?.get_password() {
        Ok(p) => Some(p),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => return Err(map_load_err(account_id, "refresh", e)),
    };
    Ok((access, refresh))
}

/// Remove all keyring entries for an account (best-effort on missing entries).
pub fn delete_tokens(account_id: &str) -> Result<()> {
    for user in [access_user(account_id), refresh_user(account_id)] {
        match entry(&user)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                return Err(Error::Other(format!(
                    "Could not remove stored credentials for account {account_id}: {e}"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_user_names_are_stable() {
        assert_eq!(access_user("abc-123"), "account:abc-123:access");
        assert_eq!(refresh_user("abc-123"), "account:abc-123:refresh");
    }
}
