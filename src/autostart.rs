//! Start-with-Windows integration via the per-user Run key:
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//!
//! No administrator rights are required for the per-user key. The registry
//! access sits behind a small trait so the enable/disable/query logic can be
//! unit-tested with an in-memory fake.

use anyhow::Result;

/// Registry subkey and value name used for the autostart entry.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "MechKeyboard";

/// Abstraction over the registry so tests can substitute a fake backend.
pub trait RunKeyStore {
    fn get(&self) -> Result<Option<String>>;
    fn set(&self, command: &str) -> Result<()>;
    fn remove(&self) -> Result<()>;
}

/// The command string stored in the Run key: the quoted path to this exe.
pub fn autostart_command() -> Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\"", exe.display()))
}

/// High-level API: enable or disable autostart via the given store.
pub fn set_autostart<S: RunKeyStore>(store: &S, enable: bool) -> Result<()> {
    if enable {
        let cmd = autostart_command()?;
        store.set(&cmd)
    } else {
        store.remove()
    }
}

/// Whether an autostart entry currently exists.
pub fn is_autostart_enabled<S: RunKeyStore>(store: &S) -> bool {
    matches!(store.get(), Ok(Some(_)))
}

/// Real Windows registry implementation.
#[cfg(windows)]
pub mod win {
    use super::*;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    pub struct RegistryRunKey;

    impl RegistryRunKey {
        pub fn new() -> Self {
            RegistryRunKey
        }

        fn open_write(&self) -> Result<RegKey> {
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            // create_subkey opens the key if it exists, creating it otherwise.
            let (key, _) = hkcu.create_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)?;
            Ok(key)
        }
    }

    impl RunKeyStore for RegistryRunKey {
        fn get(&self) -> Result<Option<String>> {
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let key = match hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) {
                Ok(k) => k,
                Err(_) => return Ok(None),
            };
            match key.get_value::<String, _>(VALUE_NAME) {
                Ok(v) => Ok(Some(v)),
                Err(_) => Ok(None),
            }
        }

        fn set(&self, command: &str) -> Result<()> {
            let key = self.open_write()?;
            key.set_value(VALUE_NAME, &command)?;
            Ok(())
        }

        fn remove(&self) -> Result<()> {
            let key = self.open_write()?;
            // Ignore "not found" errors so remove is idempotent.
            let _ = key.delete_value(VALUE_NAME);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// In-memory fake standing in for the registry value.
    struct FakeStore {
        value: RefCell<Option<String>>,
    }

    impl FakeStore {
        fn new() -> Self {
            Self {
                value: RefCell::new(None),
            }
        }
    }

    impl RunKeyStore for FakeStore {
        fn get(&self) -> Result<Option<String>> {
            Ok(self.value.borrow().clone())
        }
        fn set(&self, command: &str) -> Result<()> {
            *self.value.borrow_mut() = Some(command.to_string());
            Ok(())
        }
        fn remove(&self) -> Result<()> {
            *self.value.borrow_mut() = None;
            Ok(())
        }
    }

    #[test]
    fn enable_then_disable() {
        let store = FakeStore::new();
        assert!(!is_autostart_enabled(&store));

        // Enabling with an explicit command via the store.
        store.set("\"C:\\app.exe\"").unwrap();
        assert!(is_autostart_enabled(&store));
        assert_eq!(store.get().unwrap().as_deref(), Some("\"C:\\app.exe\""));

        set_autostart(&store, false).unwrap();
        assert!(!is_autostart_enabled(&store));
    }

    #[test]
    fn remove_is_idempotent() {
        let store = FakeStore::new();
        set_autostart(&store, false).unwrap();
        set_autostart(&store, false).unwrap();
        assert!(!is_autostart_enabled(&store));
    }

    #[test]
    fn value_name_and_key_are_stable() {
        assert_eq!(VALUE_NAME, "MechKeyboard");
        assert!(RUN_KEY.ends_with(r"CurrentVersion\Run"));
    }
}
