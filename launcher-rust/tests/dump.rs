#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tests on the player's own dump of the game: `SPARTAN_GAME_DIR` names the folder with
//! `eboot.bin`. Without it every test here returns at once (CI runs only the synthetic tests).
//! Nothing here writes into the game folder.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use spartanport_launcher::core::launch::python_command;
use spartanport_launcher::core::patches::{
    compile_patches, eboot_image, eboot_segments, write_game_patches, PatchJob, Write,
};
use spartanport_launcher::runner::glyphs;
use spartanport_launcher::runner::prepare::read_self;

/// The game folder of `SPARTAN_GAME_DIR`, when it holds an `eboot.bin`.
fn dump() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("SPARTAN_GAME_DIR")?);
    dir.join("eboot.bin").is_file().then_some(dir)
}

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
}

fn spartan() -> PathBuf {
    repo("patches/CUSA01623.xml")
}

fn metadata_names(xml: &Path) -> Vec<String> {
    let text = fs::read_to_string(xml).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name("Metadata"))
        .map(|m| m.attribute("Name").unwrap().to_string())
        .collect()
}

fn by_address(writes: &[Write]) -> HashMap<u64, Vec<u8>> {
    writes
        .iter()
        .map(|w| (w.address, w.bytes.clone()))
        .collect()
}

/// Where the rel32 at `data[at..at + 4]` points, measured from `end`.
fn rel(data: &[u8], at: usize, end: u64) -> u64 {
    let disp = i32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    end.checked_add_signed(i64::from(disp)).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn spartanport_signatures_in_the_reference_and_a_moved_build() {
    let Some(game) = dump() else {
        return;
    };
    let elf = read_self(&game.join("eboot.bin")).unwrap().elf;
    let image = eboot_image(&elf).unwrap();
    // As patches.py: .bss included (memsz).
    let segments = eboot_segments(&elf).unwrap();
    let compile = |name: &str, version: Option<&str>, image: Option<&[u8]>| {
        compile_patches(&spartan(), &[name.to_string()], version, &segments, image)
            .unwrap()
            .0
    };
    // The reference build: the signatures give exactly the Address writes.
    for name in metadata_names(&spartan()) {
        assert_eq!(
            compile(&name, None, Some(&image)),
            compile(&name, Some("01.02"), None),
            "{name}"
        );
    }
    // A build whose code moved like the full 01.02 update's: +0x120 from 0x1f0000 to 0x210000,
    // data unchanged (the bytes taken out keep the data at its address).
    let mut moved = image[..0x1f_0000].to_vec();
    moved.extend(vec![0; 0x120]);
    moved.extend(&image[0x1f_0000..0x21_0000]);
    moved.extend(&image[0x21_0120..]);
    assert_eq!(moved.len(), image.len());
    let shift = |at: u64| {
        if (0x1f_0000..0x21_0000).contains(&at) {
            at + 0x120
        } else {
            at
        }
    };
    for name in [
        "Resolution Patch - 4K",
        "Resolution Patch - 8K",
        "Skip Intro",
    ] {
        let reference = compile(name, None, Some(&image));
        let found = compile(name, None, Some(&moved));
        let found: Vec<u64> = found.iter().map(|w| w.address).collect();
        let expected: Vec<u64> = reference.iter().map(|w| shift(w.address)).collect();
        assert_eq!(found, expected, "{name}");
    }
    // Skip Intro's cave moved: the boot call into it is recomputed, the flag (data) is not.
    let writes = by_address(&compile("Skip Intro", None, Some(&moved)));
    assert_eq!(rel(&writes[&0x01_b36a], 1, 0x01_b36f), shift(0x21_2170));
    // The getter (not moved) and the cave share it.
    let flag = rel(&writes[&0x0c_8440], 2, 0x0c_844a);
    let cave = shift(0x21_2170);
    assert_eq!(rel(&writes[&cave], 2, cave + 10), flag);
}

#[test]
fn spartanport_skip_intro_original_bytes() {
    let Some(game) = dump() else {
        return;
    };
    let info = read_self(&game.join("eboot.bin")).unwrap();
    let text = info
        .program_headers
        .iter()
        .find(|p| p.kind == 1 && p.flags & 1 != 0)
        .unwrap();
    let nops = "90".repeat(15);
    for (offset, value) in [
        (0x18_4950, "7534"),
        (0x0c_8440, "488d05955b38008b00c3660f1f440000"),
        (0x01_b36a, "e821990900"),
        (0x21_2170, nops.as_str()),
    ] {
        let at = usize::try_from(text.offset + offset - text.vaddr).unwrap();
        assert_eq!(
            hex(&info.elf[at..at + value.len() / 2]),
            value,
            "{offset:#x}"
        );
    }
}

/// A third-party patch file and its selection: the Python and the Rust must both apply them on
/// the reference build.
const EXTERNAL: &str = r#"<Patch>
  <TitleID><ID>CUSA01623</ID></TitleID>
  <Metadata Name="On" AppVer="01.02" AppElf="eboot.bin" isEnabled="true"><PatchList><Line Type="bytes" Address="0x00401000" Value="9090"/></PatchList></Metadata>
  <Metadata Name="Off" AppVer="01.02" AppElf="eboot.bin"><PatchList><Line Type="bytes32" Address="0x00402000" Value="0x12345678"/></PatchList></Metadata>
  <Metadata Name="Mask" AppVer="01.02" AppElf="eboot.bin" isEnabled="true"><PatchList><Line Type="mask" Value="90 ?? 90" Offset="0"/></PatchList></Metadata>
</Patch>"#;

/// One start's patch step: the `--extra` names, `SPARTAN_PATCHES_ONLY`, the mods' patch files
/// and whether the third-party folder holds [`EXTERNAL`].
struct Case {
    name: &'static str,
    extra: &'static str,
    only: bool,
    mods: Vec<PathBuf>,
    external: bool,
}

/// Runs `scripts/patches.py` as `run.py` does (`run_script('patches.py', '--out', out, '--extra',
/// SPARTAN_PATCHES, '--game-dir', game, '--patches-dir', ..., '--patches-config', ...,
/// '--mod-patches', ...)` with `SPARTAN_PATCHES_ONLY` and `PYTHONIOENCODING=utf-8`). `None` when no
/// interpreter starts.
fn python_patches(
    case: &Case,
    out: &Path,
    game: &Path,
    patches_dir: &Path,
    config: &Path,
) -> Option<()> {
    let env = |key: &str| std::env::var(key).ok();
    let command = python_command(&env).ok()?;
    let (program, args) = command.split_first()?;
    let mut python = Command::new(program);
    python
        .args(args)
        .arg(repo("scripts/patches.py"))
        .arg("--out")
        .arg(out)
        .arg("--extra")
        .arg(case.extra)
        .arg("--game-dir")
        .arg(game)
        .arg("--patches-dir")
        .arg(patches_dir)
        .arg("--patches-config")
        .arg(config);
    for path in &case.mods {
        python.arg("--mod-patches").arg(path);
    }
    python
        .env("SPARTAN_PATCHES_ONLY", if case.only { "1" } else { "0" })
        .env("PYTHONIOENCODING", "utf-8");
    let output = python.output().ok()?;
    assert!(
        output.status.success(),
        "{}: patches.py failed:\n{}",
        case.name,
        String::from_utf8_lossy(&output.stderr)
    );
    Some(())
}

#[test]
fn real_patches_bin_matches_python() {
    let Some(game) = dump() else {
        return;
    };
    let elf = read_self(&game.join("eboot.bin")).unwrap().elf;
    let cases = [
        Case {
            name: "defaults",
            extra: "",
            only: false,
            mods: Vec::new(),
            external: false,
        },
        Case {
            name: "chosen",
            extra: "Resolution Patch - 4K;Frame Rate Patch - 120 FPS;Skip Intro;Skip Any Video With X Button",
            only: true,
            mods: Vec::new(),
            external: true,
        },
        Case {
            name: "mod",
            extra: "Resolution Patch - 3440x1440;Frame Rate Patch - 144 FPS",
            only: true,
            mods: vec![repo("extras/mods/Ultrawide/patches/Ultrawide.xml")],
            external: false,
        },
    ];
    for case in cases {
        let dir = tempfile::tempdir().unwrap();
        let patches_dir = dir.path().join("patches");
        fs::create_dir_all(&patches_dir).unwrap();
        let config = dir.path().join("patches.json");
        if case.external {
            fs::write(patches_dir.join("extra.xml"), EXTERNAL).unwrap();
            fs::write(&config, r#"{"enabled": ["extra.xml/Off"]}"#).unwrap();
        }
        let (python, rust) = (dir.path().join("python"), dir.path().join("rust"));
        for out in [&python, &rust] {
            fs::create_dir_all(out).unwrap();
            fs::write(out.join("eboot.elf"), &elf).unwrap();
        }
        if python_patches(&case, &python, &game, &patches_dir, &config).is_none() {
            return;
        }
        let extra: Vec<String> = case.extra.split(';').map(str::to_string).collect();
        let job = PatchJob {
            out: &rust,
            game: &game,
            builtin_dir: &repo("patches"),
            mod_patches: &case.mods,
            extra: &extra,
            only: case.only,
            patches_dir: Some(&patches_dir),
            patches_config: Some(&config),
        };
        let mut log = Vec::new();
        write_game_patches(&job, &mut log).unwrap();
        let log = String::from_utf8(log).unwrap();
        for name in ["patches.bin", "patch_status.json"] {
            let ours = fs::read(rust.join(name)).unwrap();
            let theirs = fs::read(python.join(name)).unwrap();
            assert!(ours == theirs, "{}: {name} differs\n{log}", case.name);
        }
        if case.external {
            assert!(
                log.contains("Patches: external extra.xml/On (1 writes)\n"),
                "{log}"
            );
            assert!(
                log.contains("Patches: external extra.xml/Off (1 writes)\n"),
                "{log}"
            );
        }
        if !case.mods.is_empty() {
            // The Python writes it in text mode: CRLF on Windows.
            let ours = fs::read_to_string(rust.join("patches-merged.xml")).unwrap();
            let theirs = fs::read_to_string(python.join("patches-merged.xml")).unwrap();
            assert!(
                ours == theirs.replace("\r\n", "\n"),
                "{}: patches-merged.xml differs",
                case.name
            );
        }
    }
}

/// Runs `glyphs.build(game, out, style)` of `scripts/glyphs.py` (`None` when no interpreter starts).
fn python_glyphs(game: &Path, out: &Path, style: &str) -> Option<()> {
    let env = |key: &str| std::env::var(key).ok();
    let command = python_command(&env).ok()?;
    let (program, args) = command.split_first()?;
    let script = "import sys; sys.path.insert(0, sys.argv[1]); import glyphs; glyphs.build(sys.argv[2], sys.argv[3], sys.argv[4])";
    let output = Command::new(program)
        .args(args)
        .args(["-B", "-c", script])
        .arg(repo("scripts"))
        .arg(game)
        .arg(out)
        .arg(style)
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .ok()?;
    assert!(
        output.status.success(),
        "glyphs.build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(())
}

/// The DXT blocks of the texture whose DDS header is at `dds`, level by level: (offset, size).
fn dxt_blocks(data: &[u8], dds: usize, end: usize) -> Vec<(usize, usize)> {
    let height = u32::from_le_bytes(data[dds + 12..dds + 16].try_into().unwrap());
    let width = u32::from_le_bytes(data[dds + 16..dds + 20].try_into().unwrap());
    let block = if &data[dds + 84..dds + 88] == b"DXT5" {
        16
    } else {
        8
    };
    let (mut offset, mut level, mut blocks) = (dds + 128, 0u32, Vec::new());
    while offset < end {
        let w = (width.checked_shr(level).unwrap_or(0)).max(1) as usize;
        let h = (height.checked_shr(level).unwrap_or(0)).max(1) as usize;
        let size = w.div_ceil(4) * h.div_ceil(4) * block;
        if offset + size > end {
            break;
        }
        blocks.extend((0..size / block).map(|i| (offset + i * block, block)));
        offset += size;
        level += 1;
        if w == 1 && h == 1 {
            break;
        }
    }
    blocks
}

#[test]
fn real_glyph_wads_match_python_outside_the_painted_blocks() {
    let Some(game) = dump() else {
        return;
    };
    let assets = glyphs::Assets::in_port(&repo(""));
    for style in ["xbox", "nintendo", "keyboard"] {
        let dir = tempfile::tempdir().unwrap();
        let (python, rust) = (dir.path().join("python"), dir.path().join("rust"));
        if python_glyphs(&game, &python, style).is_none() {
            return;
        }
        let mounts = glyphs::build(&game, &rust, style, None, "", "", &assets, &mut |line| {
            println!("{line}")
        })
        .unwrap();
        let theirs = fs::read(python.join("glyphs/stamp.json")).unwrap();
        assert_eq!(
            fs::read(rust.join("glyphs/stamp.json")).unwrap(),
            theirs,
            "{style}: stamp.json"
        );
        for name in glyphs::WADS {
            let file = Path::new(name).file_name().unwrap();
            let (ours, theirs) = (
                rust.join("glyphs").join(file),
                python.join("glyphs").join(file),
            );
            assert_eq!(
                ours.is_file(),
                theirs.is_file(),
                "{style}: {name} made by one side only"
            );
            if !theirs.is_file() {
                continue;
            }
            assert!(
                mounts.contains_key(&format!("/app0/{name}")),
                "{style}: {name} not mounted"
            );
            let original = fs::read(game.join(name)).unwrap();
            let (ours, theirs) = (fs::read(ours).unwrap(), fs::read(theirs).unwrap());
            assert_eq!(ours.len(), theirs.len(), "{style}: {name} size");
            assert_eq!(theirs.len(), original.len(), "{style}: {name} size");
            // Both sides repaint the same entries; inside those, a DXT block either side
            // re-encoded is exempt (the text's edges move which blocks a lyre re-encode touches).
            // Every other byte of both copies is the game's own.
            let mut painted = vec![false; ours.len()];
            for entry in glyphs::entries(&original) {
                let (start, end) = (entry.offset + 0x40, entry.offset + 0x40 + entry.size);
                let changed = |copy: &[u8]| copy[start..end] != original[start..end];
                assert_eq!(
                    changed(&ours),
                    changed(&theirs),
                    "{style}: {name}: {} repainted by one side only (Rust: {})",
                    entry.name,
                    changed(&ours)
                );
                if !changed(&theirs) {
                    continue;
                }
                let Some(dds) = original[start..end]
                    .windows(4)
                    .position(|w| w == b"DDS ")
                    .map(|i| i + start)
                else {
                    continue;
                };
                if !matches!(&original[dds + 84..dds + 88], b"DXT1" | b"DXT5") {
                    continue;
                }
                for (at, size) in dxt_blocks(&original, dds, end) {
                    let changed = |copy: &[u8]| copy[at..at + size] != original[at..at + size];
                    if changed(&ours) || changed(&theirs) {
                        painted[at..at + size].fill(true);
                    }
                }
            }
            let differing = (0..ours.len())
                .find(|i| !painted[*i] && (ours[*i] != original[*i] || theirs[*i] != original[*i]));
            assert_eq!(
                differing, None,
                "{style}: {name} differs from the game outside the painted blocks"
            );
        }
    }
}
