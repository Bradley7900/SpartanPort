//! Steam's folders and its binary VDF (the non-Steam shortcuts file).
//!
//! Binary VDF: 0 = map (key, entries, 8), 1 = string, 2 = uint32, 8 = end of the map.

use crate::core::platform;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A value of the binary VDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vdf {
    Map(IndexMap<String, Vdf>),
    Str(String),
    U32(u32),
}

/// Why a shortcut could not be added.
#[derive(Debug)]
pub enum SteamError {
    /// Steam is open (it rewrites `shortcuts.vdf` on exit).
    Running,
    /// No Steam account folder was found.
    NoSteam,
    /// The game dump has no background picture in this folder.
    NoArt(PathBuf),
    Io(io::Error),
}

impl From<io::Error> for SteamError {
    fn from(e: io::Error) -> Self {
        SteamError::Io(e)
    }
}

impl fmt::Display for SteamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SteamError::Running => f.write_str("running"),
            SteamError::NoSteam => f.write_str("no_steam"),
            SteamError::NoArt(path) => write!(f, "no_art: {}", path.display()),
            SteamError::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SteamError {}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn truncated(&self) -> String {
        format!("truncated VDF at {}", self.pos)
    }

    fn byte(&mut self) -> Result<u8, String> {
        let b = *self.data.get(self.pos).ok_or_else(|| self.truncated())?;
        self.pos += 1;
        Ok(b)
    }

    fn text(&mut self) -> Result<String, String> {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| self.truncated())?;
        let value = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.pos += len + 1;
        Ok(value)
    }

    fn entries(&mut self) -> Result<IndexMap<String, Vdf>, String> {
        let mut result = IndexMap::new();
        loop {
            let kind = self.byte()?;
            if kind == 8 {
                return Ok(result);
            }
            if kind > 2 {
                return Err(format!("unknown VDF type {kind} at {}", self.pos));
            }
            let key = self.text()?;
            let value = match kind {
                0 => Vdf::Map(self.entries()?),
                1 => Vdf::Str(self.text()?),
                _ => {
                    let bytes = self
                        .data
                        .get(self.pos..self.pos + 4)
                        .ok_or_else(|| self.truncated())?;
                    self.pos += 4;
                    Vdf::U32(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                }
            };
            result.insert(key, value);
        }
    }
}

/// Reads a binary VDF file.
pub fn parse_vdf(data: &[u8]) -> Result<IndexMap<String, Vdf>, String> {
    Reader { data, pos: 0 }.entries()
}

fn write_entries(out: &mut Vec<u8>, map: &IndexMap<String, Vdf>) {
    for (key, value) in map {
        match value {
            Vdf::Map(inner) => {
                out.push(0);
                out.extend_from_slice(key.as_bytes());
                out.push(0);
                write_entries(out, inner);
            }
            Vdf::Str(text) => {
                out.push(1);
                out.extend_from_slice(key.as_bytes());
                out.push(0);
                out.extend_from_slice(text.as_bytes());
                out.push(0);
            }
            Vdf::U32(n) => {
                out.push(2);
                out.extend_from_slice(key.as_bytes());
                out.push(0);
                out.extend_from_slice(&n.to_le_bytes());
            }
        }
    }
    out.push(8);
}

/// Writes a binary VDF file.
pub fn dump_vdf(tree: &IndexMap<String, Vdf>) -> Vec<u8> {
    let mut out = Vec::new();
    write_entries(&mut out, tree);
    out
}

/// The 32-bit id Steam gives a non-Steam game: the CRC of its quoted exe and name, top bit set.
pub fn shortcut_appid(exe: &str, name: &str) -> u32 {
    crc32fast::hash(format!("{exe}{name}").as_bytes()) | 0x8000_0000
}

/// The command that lists Steam's process.
pub fn steam_query(windows: bool) -> Vec<&'static str> {
    if windows {
        vec!["tasklist", "/FI", "IMAGENAME eq steam.exe", "/NH"]
    } else {
        vec!["pgrep", "-x", "steam"]
    }
}

/// Reads the answer of [`steam_query`].
pub fn running_from(windows: bool, code: Option<i32>, stdout: &str) -> bool {
    if windows {
        stdout.to_lowercase().contains("steam.exe")
    } else {
        code == Some(0)
    }
}

/// Whether Steam is open. A command that cannot start counts as not running.
pub fn steam_running() -> bool {
    let windows = cfg!(windows);
    let query = steam_query(windows);
    let mut cmd = Command::new(query[0]);
    cmd.args(&query[1..]);
    platform::no_window(&mut cmd);
    match cmd.output() {
        Ok(out) => running_from(
            windows,
            out.status.code(),
            &String::from_utf8_lossy(&out.stdout),
        ),
        Err(_) => false,
    }
}

/// Steam installation folders to look in, the registry's first on Windows.
pub fn steam_roots_from(windows: bool, registry: Option<PathBuf>, home: &Path) -> Vec<PathBuf> {
    if windows {
        let mut roots: Vec<PathBuf> = registry.into_iter().collect();
        roots.push(PathBuf::from("C:/Program Files (x86)/Steam"));
        roots.push(PathBuf::from("C:/Program Files/Steam"));
        roots
    } else {
        [
            ".local/share/Steam",
            ".steam/steam",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        ]
        .iter()
        .map(|p| home.join(p))
        .collect()
    }
}

/// [`steam_roots_from`] for this machine and user.
pub fn steam_roots() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    steam_roots_from(cfg!(windows), platform::steam_path_from_registry(), &home)
}

/// `userdata/<account>` folders of the Steam installation(s) of this user, each once.
pub fn account_folders(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for base in roots {
        let Ok(read) = std::fs::read_dir(base.join("userdata")) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for entry in entries {
            let name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let numbered =
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) && name != "0";
            if !numbered || !entry.is_dir() {
                continue;
            }
            let real = std::fs::canonicalize(&entry).unwrap_or_else(|_| entry.clone());
            if seen.insert(real) {
                found.push(entry);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use std::path::{Path, PathBuf};

    fn sample() -> IndexMap<String, Vdf> {
        let mut tags = IndexMap::new();
        tags.insert("0".to_string(), Vdf::Str("favorite".into()));
        let mut entry = IndexMap::new();
        entry.insert("appid".to_string(), Vdf::U32(0x8000_0001));
        entry.insert("AppName".to_string(), Vdf::Str("A".into()));
        entry.insert("tags".to_string(), Vdf::Map(tags));
        entry.insert("IsHidden".to_string(), Vdf::U32(0));
        let mut shortcuts = IndexMap::new();
        shortcuts.insert("0".to_string(), Vdf::Map(entry));
        let mut tree = IndexMap::new();
        tree.insert("shortcuts".to_string(), Vdf::Map(shortcuts));
        tree
    }

    #[test]
    fn round_trip_keeps_strings_numbers_and_nested_maps() {
        let tree = sample();
        let bytes = dump_vdf(&tree);
        assert_eq!(parse_vdf(&bytes), Ok(tree));
        let again = parse_vdf(&bytes).unwrap();
        assert_eq!(dump_vdf(&again), bytes);
    }

    #[test]
    fn empty_file_of_steam() {
        let bytes = b"\x00shortcuts\x00\x08\x08";
        let tree = parse_vdf(bytes).unwrap();
        let mut want = IndexMap::new();
        want.insert("shortcuts".to_string(), Vdf::Map(IndexMap::new()));
        assert_eq!(tree, want);
        assert_eq!(dump_vdf(&tree), bytes);
    }

    #[test]
    fn broken_files_are_errors() {
        assert_eq!(
            parse_vdf(b"\x05x\x00"),
            Err("unknown VDF type 5 at 1".to_string())
        );
        let err = parse_vdf(b"\x00shortcuts\x00\x01AppName\x00Spar").unwrap_err();
        assert!(err.contains("truncated"), "{err}");
    }

    #[test]
    fn app_id_is_the_crc_of_exe_and_name_with_the_top_bit() {
        assert_eq!(
            shortcut_appid("\"/home/user/SpartanPort.AppImage\"", "SpartanPort"),
            0xa02add32
        );
    }

    #[test]
    fn windows_looks_in_the_registry_folder_then_the_usual_ones() {
        let home = Path::new("/home/u");
        let win = steam_roots_from(true, Some(PathBuf::from("D:/Games/Steam")), home);
        assert_eq!(win[0], PathBuf::from("D:/Games/Steam"));
        assert!(win.contains(&PathBuf::from("C:/Program Files (x86)/Steam")));
        assert!(win.contains(&PathBuf::from("C:/Program Files/Steam")));
        assert_eq!(
            steam_roots_from(false, None, home),
            vec![
                home.join(".local/share/Steam"),
                home.join(".steam/steam"),
                home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            ]
        );
    }

    #[test]
    fn steam_is_running_by_the_process_list() {
        assert!(running_from(
            true,
            Some(0),
            "steam.exe   1234 Console   1   150,000 K"
        ));
        assert!(!running_from(
            true,
            Some(0),
            "INFO: No tasks are running which match the specified criteria."
        ));
        assert!(running_from(false, Some(0), ""));
        assert!(!running_from(false, Some(1), ""));
        assert_eq!(steam_query(false)[0], "pgrep");
        assert_eq!(steam_query(true)[0], "tasklist");
    }

    #[test]
    fn accounts_are_the_numbered_folders_but_0() {
        let dir = tempfile::tempdir().unwrap();
        let users = dir.path().join("userdata");
        for name in ["1111", "2222", "0", "abc"] {
            std::fs::create_dir_all(users.join(name)).unwrap();
        }
        std::fs::write(users.join("3333"), "").unwrap();
        let missing = dir.path().join("missing");
        let found = account_folders(&[missing, dir.path().to_path_buf()]);
        assert_eq!(found, vec![users.join("1111"), users.join("2222")]);
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_root_is_counted_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        std::fs::create_dir_all(a.join("userdata/1111")).unwrap();
        std::fs::create_dir_all(a.join("userdata/2222")).unwrap();
        let b = dir.path().join("b");
        std::os::unix::fs::symlink(&a, &b).unwrap();
        let found = account_folders(&[a, b]);
        assert_eq!(found.len(), 2);
    }
}
