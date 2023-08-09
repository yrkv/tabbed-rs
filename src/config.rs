
use serde::{Deserialize, Serialize};
use lazy_static::lazy_static;

use figment::{Figment, providers::{Serialized, Format, Toml}};

use std::path::Path;
use std::path::PathBuf;
use std::env;
use std::ffi::OsString;
use std::fs::read_to_string;

//use x11rb::protocol::xproto::KeyButMask as Mod;
use x11rb_protocol::protocol::xproto::KeyButMask;
use x11rb_protocol::protocol::xproto::ModMask;


lazy_static! {
    pub static ref DEFAULT_CONFIG: Config = Config {
        keybinds: vec![
            Keybind { modifiers: vec![Mod::SHIFT, Mod::CONTROL], key: 43, action: Action::FocusDown },
            Keybind { modifiers: vec![Mod::SHIFT, Mod::CONTROL], key: 44, action: Action::ShiftDown },
            Keybind { modifiers: vec![Mod::SHIFT, Mod::CONTROL], key: 45, action: Action::ShiftUp },
            Keybind { modifiers: vec![Mod::SHIFT, Mod::CONTROL], key: 46, action: Action::FocusUp },

            Keybind { modifiers: vec![Mod::SHIFT, Mod::CONTROL], key: 22, action: Action::DetachFocused },
            Keybind { modifiers: vec![Mod::SHIFT, Mod::CONTROL], key: 9,  action: Action::DetachAll },

            Keybind { modifiers: vec![Mod::CONTROL], key: 10, action: Action::Focus(0) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 11, action: Action::Focus(1) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 12, action: Action::Focus(2) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 13, action: Action::Focus(3) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 14, action: Action::Focus(4) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 15, action: Action::Focus(5) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 16, action: Action::Focus(6) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 17, action: Action::Focus(7) },
            Keybind { modifiers: vec![Mod::CONTROL], key: 18, action: Action::Focus(8) },

            //Keybind { modifiers: vec![Mod::CONTROL], key: 19, action: Action::Focus(9) },

        ],
        auto_attach: false,
        colors: true,
        font: None,
    };
}



#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Action {
    FocusUp,
    FocusDown,
    ShiftUp,
    ShiftDown,
    Focus(usize),
    DetachFocused,
    DetachAll,
    ToggleAutoAttach,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Mod {
    SHIFT,
    LOCK,
    CONTROL,
    ALT,
    ANY,
}

impl From<Mod> for u16 {
    fn from(input: Mod) -> u16 {
        match input {
            Mod::SHIFT => 1 << 0,
            Mod::LOCK => 1 << 1,
            Mod::CONTROL => 1 << 2,
            Mod::ALT => 1 << 3,
            Mod::ANY => 1 << 15,
        }
    }
}


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Keybind {
    pub modifiers: Vec<Mod>,
    pub key: u8,
    pub action: Action,
}

impl Keybind {
    pub fn mod_mask(&self) -> ModMask {
        let a = self.modifiers.iter().map(|m| u16::from(m.clone()).into());
        a.reduce(|acc, x| acc | x).unwrap_or(ModMask::ANY)
    }

    pub fn key_but_mask(&self) -> KeyButMask {
        let a = self.modifiers.iter().map(|m| u16::from(m.clone()).into());
        a.reduce(|acc, x| acc | x).unwrap_or(KeyButMask::from(0u16))
    }
}


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub keybinds: Vec<Keybind>,
    pub auto_attach: bool,
    pub colors: bool,
    pub font: Option<String>,
}


#[derive(Debug,)]
pub enum ConfigError {
    /// Failed to open/read a specified file
    IoError(OsString, std::io::Error),
    /// Failed to parse the configuration
    FigmentError(Option<OsString>, figment::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(path, e) => {
                write!(f, "{}: {}", path.to_str().unwrap_or("unknown file"), e)
            },
            ConfigError::FigmentError(None, e) => {
                write!(f, "no file: {}", e)
            },
            ConfigError::FigmentError(Some(path), e) => {
                write!(f, "{}: {}", path.to_str().unwrap_or("unknown file"), e.kind)
            },
        }
    }
}

impl std::error::Error for ConfigError {}


/// Return the first found configuration path, if any.
///
/// The default search order is:
///     1. `RSTAB_CONFIG_PATH` environment variable if set
///     2. `$HOME/.config/rstab.toml` file if it exists
///
fn find_config() -> Option<OsString> {
    match env::var_os("RSTAB_CONFIG_PATH") {
        Some(env_path) => return Some(env_path),
        None => (),
    }

    let default_config_path = env::var_os("HOME")
        .map(|h| Path::new(&h).join(".config/rstab.toml"));

    match default_config_path {
        Some(path) => if path.exists() { return Some(path.into()); }
        None => ()
    }

    None
}

/// Find, read, and parse the configuration.
///
/// If provided, the `cli_path` is used instead of searching for it.
pub fn read_config(cli_path: &Option<PathBuf>) -> Result<Config, ConfigError> {
    let mut configment = Figment::from(Serialized::defaults(DEFAULT_CONFIG.to_owned()));

    let config_path = match cli_path {
        Some(path) => Some(path.into()),
        None => find_config()
    };

    if let Some(path) = &config_path {
        let data = match read_to_string(path) {
            Ok(data) => data,
            Err(e) => return Err(ConfigError::IoError(path.into(), e)),
        };
        configment = configment.merge(Toml::string(&data));
    }

    match configment.extract() {
        Ok(config) => Ok(config),
        Err(e) => Err(ConfigError::FigmentError(config_path, e))
    }
}


