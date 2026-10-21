//! Patches of the game's eboot: the built-in XML's metadata, the selection rules, third-party
//! patch files and the report of the last start.

use std::io;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use super::game::GameProfile;
use super::paths::Paths;

/// Resolution patches share the video arena with the texture fix: only one of them is on.
pub const EXCLUSIVE_PREFIX: &str = "Resolution Patch";
/// Fixes with nothing to choose: not offered in the launcher and always applied.
pub const ALWAYS_ON: &[&str] = &["Bug Fix - Startup Black Screen"];
/// An older build's patch file, left in `patches/` by an update.
const LEGACY_PATCH_FILE: &str = "God_of_War_III_Remastered.xml";
const FRAME_RATE_PREFIX: &str = "Frame Rate Patch";

#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{0}")]
    Xml(String),
}

/// One `Metadata` element of a patch file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchMeta {
    pub name: String,
    pub author: Option<String>,
    pub note: Option<String>,
    pub enabled_by_default: bool,
}

/// The report `patches.py` writes after a start (`patch_status.json`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PatchStatus {
    #[serde(default)]
    pub applied: Vec<String>,
    /// Patch name to the reason it was left out.
    #[serde(default)]
    pub skipped: IndexMap<String, String>,
}

/// The built-in patch file of a game profile.
pub fn builtin_xml(paths: &Paths, profile: &GameProfile) -> PathBuf {
    paths.port_dir.join("patches").join(profile.xml)
}

fn parse(text: &str) -> Result<roxmltree::Document<'_>, roxmltree::Error> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    roxmltree::Document::parse_with_options(text, options)
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.filter(|v| !v.is_empty()).map(str::to_string)
}

fn meta_of(node: roxmltree::Node) -> PatchMeta {
    PatchMeta {
        name: node.attribute("Name").unwrap_or_default().to_string(),
        author: non_empty(node.attribute("Author")),
        note: non_empty(node.attribute("Note")),
        enabled_by_default: node
            .attribute("isEnabled")
            .is_some_and(|v| v.to_lowercase() == "true"),
    }
}

fn metadata<'a, 'input>(
    doc: &'a roxmltree::Document<'input>,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    doc.descendants().filter(|n| n.has_tag_name("Metadata"))
}

/// The patches the XML has for this app version, in file order.
pub fn game_patch_metas(xml: &Path, app_version: &str) -> Result<Vec<PatchMeta>, PatchError> {
    let text = std::fs::read_to_string(xml)?;
    let doc = parse(&text).map_err(|e| PatchError::Xml(e.to_string()))?;
    Ok(metadata(&doc)
        .filter(|n| n.attribute("AppVer") == Some(app_version))
        .map(meta_of)
        .collect())
}

/// The patches the launcher offers: without the resolution and frame rate ones (they follow
/// Performance) and the always-on fixes.
pub fn selectable(metas: &[PatchMeta]) -> Vec<PatchMeta> {
    metas
        .iter()
        .filter(|m| {
            !(m.name.starts_with(EXCLUSIVE_PREFIX)
                || m.name.starts_with(FRAME_RATE_PREFIX)
                || ALWAYS_ON.contains(&m.name.as_str()))
        })
        .cloned()
        .collect()
}

/// Sort key of a size: its pixel count, then the text; anything that is not WxH goes last.
fn pixels(size: &str) -> (u128, String) {
    let (width, height) = size.split_once('x').unwrap_or((size, ""));
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if digits(width) && digits(height) {
        if let (Ok(w), Ok(h)) = (width.parse::<u128>(), height.parse::<u128>()) {
            return (w * h, size.to_string());
        }
    }
    (1 << 60, size.to_string())
}

/// Sizes ("3440x1440") of the "Resolution Patch - <size>" entries of these files, smallest first.
pub fn resolution_names(xmls: &[PathBuf]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for path in xmls {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(doc) = parse(&text) else {
            continue;
        };
        for node in metadata(&doc) {
            let name = node.attribute("Name").unwrap_or_default();
            if let Some(size) = name
                .strip_prefix(EXCLUSIVE_PREFIX)
                .and_then(|rest| rest.strip_prefix(" - "))
            {
                let size = size.trim();
                if !size.is_empty() && !found.iter().any(|f| f == size) {
                    found.push(size.to_string());
                }
            }
        }
    }
    found.sort_by_key(|s| pixels(s));
    found
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// `(key, file, metadata)` of the eboot patches for this version in `dir/*.xml` (third-party
/// shadPS4 or GoldHEN files). The key is `"<file name>/<patch name>"`, the launcher's selection
/// in `patches.json`. The built-in files are left out, and so are files whose `ID`s do not
/// include any of `ids`.
pub fn external_patches(
    dir: &Path,
    app_version: &str,
    exclude: &Path,
    ids: &[&str],
) -> Vec<(String, PathBuf, PatchMeta)> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "xml") && p.is_file())
        .collect();
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // The profile's file (patches/ in a checkout) and an older build's are not external.
        if same_file(&path, exclude) || name == "CUSA01623.xml" || name == LEGACY_PATCH_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let doc = match parse(&text) {
            Ok(doc) => doc,
            Err(error) => {
                eprintln!("Patches: {name}: {error}");
                continue;
            }
        };
        let file_ids: Vec<&str> = doc
            .descendants()
            .filter(|n| n.has_tag_name("ID"))
            .filter_map(|n| n.text())
            .map(str::trim)
            .collect();
        if !file_ids.is_empty() && !file_ids.iter().any(|id| ids.contains(id)) {
            continue;
        }
        for node in metadata(&doc) {
            if node.attribute("AppVer") == Some(app_version)
                && node.attribute("AppElf").unwrap_or("eboot.bin") == "eboot.bin"
            {
                let meta = meta_of(node);
                found.push((format!("{name}/{}", meta.name), path.clone(), meta));
            }
        }
    }
    found
}

/// The keys of the external patches that are on. `config` is the content of `patches.json`,
/// `{"enabled": [key, ...], "disabled": [key, ...]}`: a key in `enabled` is on, one in `disabled`
/// is off, any other follows the file's `isEnabled`. `enabled` wins when a key is in both.
pub fn external_enabled(found: &[(String, PathBuf, PatchMeta)], config: &Value) -> Vec<String> {
    let listed = |field: &str, key: &str| {
        config
            .get(field)
            .and_then(Value::as_array)
            .is_some_and(|list| list.iter().any(|v| v.as_str() == Some(key)))
    };
    found
        .iter()
        .filter(|(key, _, meta)| {
            listed("enabled", key) || (!listed("disabled", key) && meta.enabled_by_default)
        })
        .map(|(key, _, _)| key.clone())
        .collect()
}

/// The last start's patch report in `out_dir`; `None` when missing or not valid.
pub fn patch_status(out_dir: &Path) -> Option<PatchStatus> {
    let bytes = std::fs::read(out_dir.join("patch_status.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const GAME_XML: &str = r#"<?xml version="1.0"?>
<Patch>
  <TitleID><ID>CUSA01623</ID></TitleID>
  <Metadata Name="Bug Fix - Startup Black Screen" AppVer="01.02" isEnabled="true"/>
  <Metadata Name="Resolution Patch - 1920x1080" AppVer="01.02" isEnabled="false"/>
  <Metadata Name="Resolution Patch - 854x480" AppVer="01.02"/>
  <Metadata Name="Resolution Patch - 3440x1440" AppVer="01.02"/>
  <Metadata Name="Resolution Patch - wide" AppVer="01.02"/>
  <Metadata Name="Frame Rate Patch - 120 FPS" AppVer="01.02"/>
  <Metadata Name="Skip Intro" Author="me" Note="faster" AppVer="01.02" isEnabled="TRUE"/>
  <Metadata Name="Old Fix" Author="" Note="" AppVer="01.00" isEnabled="true"/>
</Patch>"#;

    const EXTRA_XML: &str = r#"<Patch>
  <TitleID><ID>CUSA99999</ID></TitleID>
  <Metadata Name="Other Game" AppVer="01.02"/>
</Patch>"#;

    const MINE_XML: &str = r#"<Patch>
  <TitleID><ID>CUSA01623</ID></TitleID>
  <Metadata Name="Cheat" Author="a" AppVer="01.02" isEnabled="true"/>
  <Metadata Name="Off" AppVer="01.02"/>
  <Metadata Name="Wrong Elf" AppVer="01.02" AppElf="other.elf"/>
  <Metadata Name="Wrong Version" AppVer="01.00"/>
</Patch>"#;

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn names(metas: &[PatchMeta]) -> Vec<&str> {
        metas.iter().map(|m| m.name.as_str()).collect()
    }

    #[test]
    fn metas_filter_by_app_version() {
        let dir = tempfile::tempdir().unwrap();
        let xml = write(dir.path(), "g.xml", GAME_XML);
        let metas = game_patch_metas(&xml, "01.02").unwrap();
        assert_eq!(metas.len(), 7);
        assert_eq!(metas[0].name, "Bug Fix - Startup Black Screen");
        assert!(metas[0].enabled_by_default);
        assert!(!metas[1].enabled_by_default);
        assert!(!metas[2].enabled_by_default);
        let skip = metas.iter().find(|m| m.name == "Skip Intro").unwrap();
        assert_eq!(skip.author.as_deref(), Some("me"));
        assert_eq!(skip.note.as_deref(), Some("faster"));
        assert!(skip.enabled_by_default);
        let old = game_patch_metas(&xml, "01.00").unwrap();
        assert_eq!(names(&old), ["Old Fix"]);
        assert_eq!(old[0].author, None);
        assert_eq!(old[0].note, None);
        assert!(game_patch_metas(&xml, "09.99").unwrap().is_empty());
        assert!(matches!(
            game_patch_metas(&write(dir.path(), "bad.xml", "<a>"), "01.02"),
            Err(PatchError::Xml(_))
        ));
        assert!(matches!(
            game_patch_metas(&dir.path().join("none.xml"), "01.02"),
            Err(PatchError::Io(_))
        ));
    }

    #[test]
    fn selectable_hides_resolution_frame_rate_and_always_on() {
        let dir = tempfile::tempdir().unwrap();
        let xml = write(dir.path(), "g.xml", GAME_XML);
        let metas = game_patch_metas(&xml, "01.02").unwrap();
        assert_eq!(names(&selectable(&metas)), ["Skip Intro"]);
    }

    #[test]
    fn resolution_names_sorted_by_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "a.xml", GAME_XML);
        let b = write(dir.path(), "b.xml", GAME_XML);
        let bad = write(dir.path(), "bad.xml", "<a>");
        let gone = dir.path().join("gone.xml");
        assert_eq!(
            resolution_names(&[a, bad, gone, b]),
            ["854x480", "1920x1080", "3440x1440", "wide"]
        );
    }

    #[test]
    fn external_patches_skip_builtin_and_other_games() {
        let dir = tempfile::tempdir().unwrap();
        let builtin = write(dir.path(), "builtin.xml", GAME_XML);
        write(dir.path(), "CUSA01623.xml", GAME_XML);
        write(dir.path(), LEGACY_PATCH_FILE, GAME_XML);
        write(dir.path(), "other.xml", EXTRA_XML);
        write(dir.path(), "broken.xml", "<a>");
        write(dir.path(), "notes.txt", MINE_XML);
        let mine = write(dir.path(), "mine.xml", MINE_XML);
        let found = external_patches(dir.path(), "01.02", &builtin, &["CUSA01623"]);
        let keys: Vec<&str> = found.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(keys, ["mine.xml/Cheat", "mine.xml/Off"]);
        assert_eq!(found[0].1, mine);
        assert!(found[0].2.enabled_by_default);
        assert!(external_patches(&dir.path().join("none"), "01.02", &builtin, &[]).is_empty());
    }

    #[test]
    fn external_enabled_follows_the_config() {
        let meta = |on| PatchMeta {
            name: String::new(),
            author: None,
            note: None,
            enabled_by_default: on,
        };
        let p = PathBuf::from("x.xml");
        let found = vec![
            ("x.xml/a".to_string(), p.clone(), meta(true)),
            ("x.xml/b".to_string(), p.clone(), meta(true)),
            ("x.xml/c".to_string(), p.clone(), meta(false)),
            ("x.xml/d".to_string(), p, meta(false)),
        ];
        let none = serde_json::json!({});
        assert_eq!(external_enabled(&found, &none), ["x.xml/a", "x.xml/b"]);
        let cfg = serde_json::json!({"enabled": ["x.xml/c", "x.xml/b"], "disabled": ["x.xml/b", "x.xml/a"]});
        assert_eq!(external_enabled(&found, &cfg), ["x.xml/b", "x.xml/c"]);
    }

    #[test]
    fn patch_status_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(patch_status(dir.path()), None);
        write(dir.path(), "patch_status.json", "not json");
        assert_eq!(patch_status(dir.path()), None);
    }

    #[test]
    fn patch_status_reads_applied_and_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "patch_status.json",
            r#"{"sha256":"ab","applied":["A","B"],"skipped":{"C":"site not found"}}"#,
        );
        let status = patch_status(dir.path()).unwrap();
        assert_eq!(status.applied, ["A", "B"]);
        assert_eq!(
            status.skipped.get("C").map(String::as_str),
            Some("site not found")
        );
    }

    #[test]
    fn builtin_xml_is_under_port_patches() {
        let paths = Paths::from_env(Path::new("app"), &|_| None);
        let profile = crate::core::game::game_profile(Some("CUSA01623"), None).unwrap();
        assert_eq!(
            builtin_xml(&paths, profile),
            paths.port_dir.join("patches").join(profile.xml)
        );
    }
}
