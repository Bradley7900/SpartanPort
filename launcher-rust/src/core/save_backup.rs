//! Backups of the game's saves: `<user>/savedata` copied to `<user>/savedata_backups/<time>`.
//!
//! The game start makes one before every run; the launcher lists them and restores one. A restore
//! first backs up the current saves, so it can always be undone.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};
use thiserror::Error;

/// How many backups stay after a new one is made.
pub const KEEP: usize = 10;
const STAMP: &str = "%Y%m%d-%H%M%S";

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("No save backup named {0:?}")]
    Unknown(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub fn backups_dir(user_dir: &Path) -> PathBuf {
    user_dir.join("savedata_backups")
}

/// `(YYYYMMDD-HHMMSS, n)` of `YYYYMMDD-HHMMSS[-n]`; any other name sorts by itself, before them.
fn backup_order(name: &str) -> (String, u64) {
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    let stamp_ok = name.len() >= 15
        && name.is_char_boundary(15)
        && digits(&name[..8], 8)
        && name.as_bytes()[8] == b'-'
        && digits(&name[9..15], 6);
    if stamp_ok {
        let rest = &name[15..];
        if rest.is_empty() {
            return (name[..15].to_string(), 1);
        }
        if let Some(n) = rest.strip_prefix('-') {
            if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) {
                return (name[..15].to_string(), n.parse().unwrap_or(u64::MAX));
            }
        }
    }
    (name.to_string(), 0)
}

/// Backup names, newest first.
pub fn list_backups(user_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(backups_dir(user_dir)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort_by_key(|n| std::cmp::Reverse(backup_order(n)));
    names
}

fn has_saves(folder: &Path) -> bool {
    let Ok(entries) = fs::read_dir(folder) else {
        return false;
    };
    entries.flatten().any(|e| {
        let path = e.path();
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            has_saves(&path)
        } else {
            path.is_file()
        }
    })
}

fn prune(user_dir: &Path, keep: usize) {
    for name in list_backups(user_dir).into_iter().skip(keep) {
        let _ = fs::remove_dir_all(backups_dir(user_dir).join(name));
    }
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Copies savedata into a fresh, uniquely named folder without pruning.
fn copy_backup(user_dir: &Path, now: Option<SystemTime>) -> io::Result<Option<PathBuf>> {
    let savedata = user_dir.join("savedata");
    if !has_saves(&savedata) {
        return Ok(None);
    }
    let root = backups_dir(user_dir);
    fs::create_dir_all(&root)?;
    let moment: DateTime<Local> = now.map_or_else(Local::now, DateTime::from);
    let base = moment.format(STAMP).to_string();
    // Pruning may have removed the unsuffixed folder or left gaps. Keep the sequence
    // increasing so a fresh backup cannot sort behind older ones.
    let mut n = 1;
    for entry in fs::read_dir(&root)?.flatten() {
        let (stamp, sequence) = backup_order(&entry.file_name().to_string_lossy());
        if stamp == base {
            n = n.max(sequence + 1);
        }
    }
    let target = root.join(if n == 1 { base } else { format!("{base}-{n}") });
    copy_tree(&savedata, &target)?;
    Ok(Some(target))
}

/// Copies savedata; returns the new folder, or `None` when there is nothing to save.
pub fn backup(user_dir: &Path, now: Option<SystemTime>) -> io::Result<Option<PathBuf>> {
    let made = copy_backup(user_dir, now)?;
    if made.is_some() {
        prune(user_dir, KEEP);
    }
    Ok(made)
}

/// Replaces savedata with backup `name`; returns the backup of the saves it replaced.
pub fn restore(
    user_dir: &Path,
    name: &str,
    now: Option<SystemTime>,
) -> Result<Option<PathBuf>, BackupError> {
    if name.is_empty() || !list_backups(user_dir).iter().any(|n| n == name) {
        return Err(BackupError::Unknown(name.to_string()));
    }
    let source = backups_dir(user_dir).join(name);
    let savedata = user_dir.join("savedata");
    // Pruning waits until the copy is done: the restored backup may be the oldest one.
    let safety = copy_backup(user_dir, now)?;
    let staging = user_dir.join("savedata.restoring");
    let _ = fs::remove_dir_all(&staging);
    copy_tree(&source, &staging)?;
    let _ = fs::remove_dir_all(&savedata);
    fs::rename(&staging, &savedata)?;
    prune(user_dir, KEEP);
    Ok(safety)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Local, TimeZone};
    use std::fs;
    use std::path::Path;
    use std::time::SystemTime;

    struct Fixture {
        _temp: tempfile::TempDir,
        user: PathBuf,
        save: PathBuf,
        clock: chrono::DateTime<Local>,
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().to_path_buf();
        let save = user.join("savedata/1/CUSA01623/AUTOSAVE/data.bin");
        let clock = Local.with_ymd_and_hms(2026, 10, 9, 18, 0, 0).unwrap();
        Fixture {
            _temp: temp,
            user,
            save,
            clock,
        }
    }

    impl Fixture {
        fn tick(&mut self) -> Option<SystemTime> {
            self.clock += Duration::minutes(1);
            Some(self.clock.into())
        }
        fn now(&self) -> Option<SystemTime> {
            Some(self.clock.into())
        }
        fn relative(&self) -> PathBuf {
            self.save
                .strip_prefix(self.user.join("savedata"))
                .unwrap()
                .to_path_buf()
        }
        fn names(&self) -> Vec<String> {
            list_backups(&self.user)
        }
    }

    fn name_of(path: &Path) -> String {
        path.file_name().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn copies_savedata() {
        let mut f = fixture();
        write(&f.save, "v1");
        let at = f.tick();
        let made = backup(&f.user, at).unwrap().unwrap();
        assert_eq!(name_of(&made), "20261009-180100");
        assert_eq!(fs::read_to_string(made.join(f.relative())).unwrap(), "v1");
    }

    #[test]
    fn no_backup_without_saves() {
        let mut f = fixture();
        let at = f.tick();
        assert!(backup(&f.user, at).unwrap().is_none());
        fs::create_dir(f.user.join("savedata")).unwrap();
        let at = f.tick();
        assert!(backup(&f.user, at).unwrap().is_none());
        assert!(list_backups(&f.user).is_empty());
    }

    #[test]
    fn same_second_gets_unique_name() {
        let f = fixture();
        write(&f.save, "v1");
        let first = backup(&f.user, f.now()).unwrap().unwrap();
        let second = backup(&f.user, f.now()).unwrap().unwrap();
        assert_ne!(first, second);
        assert_eq!(f.names().len(), 2);
    }

    #[test]
    fn same_second_backups_sort_by_creation_sequence() {
        let f = fixture();
        write(&f.save, "v1");
        let made: Vec<String> = (0..12)
            .map(|_| name_of(&copy_backup(&f.user, f.now()).unwrap().unwrap()))
            .collect();
        let mut newest_first = made.clone();
        newest_first.reverse();
        assert_eq!(f.names(), newest_first);
    }

    #[test]
    fn same_second_retention_keeps_latest_contents_and_unique_names() {
        let f = fixture();
        let mut made = Vec::new();
        for index in 0..14 {
            write(&f.save, &format!("version {index}"));
            let path = backup(&f.user, f.now()).unwrap().unwrap();
            assert!(
                path.is_dir(),
                "the backup just created must survive pruning"
            );
            made.push(path);
        }
        let unique: std::collections::HashSet<_> = made.iter().collect();
        assert_eq!(unique.len(), made.len());
        let expected: Vec<&PathBuf> = made[made.len() - KEEP..].iter().rev().collect();
        let names: Vec<String> = expected.iter().map(|p| name_of(p)).collect();
        assert_eq!(f.names(), names);
        let relative = f.relative();
        let contents: Vec<String> = expected
            .iter()
            .map(|p| fs::read_to_string(p.join(&relative)).unwrap())
            .collect();
        let want: Vec<String> = (4..14).rev().map(|i| format!("version {i}")).collect();
        assert_eq!(contents, want);
    }

    #[test]
    fn keeps_newest_ten() {
        let mut f = fixture();
        write(&f.save, "v");
        let made: Vec<PathBuf> = (0..12)
            .map(|_| {
                let at = f.tick();
                backup(&f.user, at).unwrap().unwrap()
            })
            .collect();
        let kept = f.names();
        assert_eq!(kept.len(), KEEP);
        assert_eq!(kept[0], name_of(&made[11]));
        assert!(!made[0].exists());
        assert!(!made[1].exists());
    }

    #[test]
    fn same_second_restore_keeps_the_safety_backup() {
        let f = fixture();
        for index in 0..12 {
            write(&f.save, &format!("version {index}"));
            backup(&f.user, f.now()).unwrap();
        }
        let oldest = f.names().pop().unwrap();
        write(&f.save, "current progress");
        let safety = restore(&f.user, &oldest, f.now()).unwrap().unwrap();
        assert_eq!(fs::read_to_string(&f.save).unwrap(), "version 2");
        assert_eq!(
            fs::read_to_string(safety.join(f.relative())).unwrap(),
            "current progress"
        );
        assert_eq!(f.names()[0], name_of(&safety));
        assert_eq!(f.names().len(), KEEP);
    }

    #[test]
    fn restore_backs_up_current_first() {
        let mut f = fixture();
        write(&f.save, "old");
        let at = f.tick();
        let old = backup(&f.user, at).unwrap().unwrap();
        write(&f.save, "new");
        write(&f.save.parent().unwrap().join("extra.bin"), "x");
        let at = f.tick();
        let safety = restore(&f.user, &name_of(&old), at).unwrap().unwrap();
        assert_eq!(fs::read_to_string(&f.save).unwrap(), "old");
        assert!(!f.save.parent().unwrap().join("extra.bin").exists());
        assert_eq!(
            fs::read_to_string(safety.join(f.relative())).unwrap(),
            "new"
        );
    }

    #[test]
    fn restore_oldest_when_full() {
        let mut f = fixture();
        write(&f.save, "oldest");
        let at = f.tick();
        let oldest = backup(&f.user, at).unwrap().unwrap();
        write(&f.save, "later");
        for _ in 0..KEEP - 1 {
            let at = f.tick();
            backup(&f.user, at).unwrap();
        }
        let at = f.tick();
        restore(&f.user, &name_of(&oldest), at).unwrap();
        assert_eq!(fs::read_to_string(&f.save).unwrap(), "oldest");
        assert_eq!(f.names().len(), KEEP);
    }

    #[test]
    fn restore_rejects_unknown_or_unsafe_names() {
        let mut f = fixture();
        write(&f.save, "v");
        for name in ["missing", "..", "../savedata", ""] {
            let at = f.tick();
            assert!(matches!(
                restore(&f.user, name, at),
                Err(BackupError::Unknown(_))
            ));
        }
        assert_eq!(fs::read_to_string(&f.save).unwrap(), "v");
    }
}
