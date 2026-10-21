//! The in-memory log ring, the run log file and the output-line classifiers.

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Whole lines the launcher log keeps; older ones are dropped.
pub const MAX_LOG_LINES: usize = 6000;

/// A ring of whole lines plus the partial line still waiting for its `\n`.
#[derive(Debug, Clone)]
pub struct LogBuffer {
    max_lines: usize,
    lines: VecDeque<String>,
    partial: String,
}

impl LogBuffer {
    pub fn new(max_lines: usize) -> Self {
        Self {
            max_lines,
            lines: VecDeque::new(),
            partial: String::new(),
        }
    }

    /// Appends `text`. Each `\n` completes a line (one trailing `\r` is stripped); the text after
    /// the last `\n` stays as a partial line that the next push continues.
    pub fn push_text(&mut self, text: &str) {
        let mut rest = text;
        while let Some(pos) = rest.find('\n') {
            self.partial.push_str(&rest[..pos]);
            if self.partial.ends_with('\r') {
                self.partial.pop();
            }
            self.lines.push_back(std::mem::take(&mut self.partial));
            rest = &rest[pos + 1..];
        }
        self.partial.push_str(rest);
        while self.lines.len() > self.max_lines {
            self.lines.pop_front();
        }
    }

    /// The whole lines, then the partial line when it is not empty.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines
            .iter()
            .map(String::as_str)
            .chain((!self.partial.is_empty()).then_some(self.partial.as_str()))
    }

    /// The lines joined with `\n`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.lines().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(line);
        }
        out
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.partial.clear();
    }

    /// Whole lines, plus one when a non-empty partial line exists.
    pub fn len(&self) -> usize {
        self.lines.len() + usize::from(!self.partial.is_empty())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The run log file. After any failure it stops writing and never fails again, so the game
/// still starts and its output is still drained.
#[derive(Debug)]
pub struct RunLog(Option<File>);

impl RunLog {
    /// Creates the folder, truncates the file; a closed log when that fails.
    pub fn open(path: &Path) -> RunLog {
        let opened = (|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            File::create(path)
        })();
        RunLog(opened.ok())
    }

    pub fn is_open(&self) -> bool {
        self.0.is_some()
    }

    /// Writes and flushes `text`; on any error the file is dropped and later writes do nothing.
    pub fn write(&mut self, text: &str) {
        if let Some(file) = &mut self.0 {
            if file
                .write_all(text.as_bytes())
                .and_then(|()| file.flush())
                .is_err()
            {
                self.0 = None;
            }
        }
    }
}

/// Why the game closed, when an output line says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitHint {
    GuestMemory,
    DumpIncomplete,
}

impl ExitHint {
    /// The English advice; pass it through `tr` for the UI language.
    pub fn text_en(&self) -> &'static str {
        match self {
            ExitHint::GuestMemory => {
                "Windows could not reserve the memory the game needs. Close other programs, or let the \
                 Windows page file grow (for example 16384 MB initial, 32768 MB maximum), and start again."
            }
            ExitHint::DumpIncomplete => {
                "The game stopped while reading its files: the game dump may be incomplete. Extract the pkg \
                 again and check that the game folder is about 40 GB."
            }
        }
    }
}

pub fn exit_hint(line: &str) -> Option<ExitHint> {
    if line.contains("of guest memory (error ") {
        Some(ExitHint::GuestMemory)
    } else if line.contains("guest raised debug exception code=0xa0020001") {
        Some(ExitHint::DumpIncomplete)
    } else {
        None
    }
}

/// What an output line says about the game's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStatus {
    Running,
    Restarting,
}

pub fn line_status(line: &str) -> Option<LineStatus> {
    if line.contains("Entering original x86-64 code") {
        Some(LineStatus::Running)
    } else if line.contains("restarting through run.py") {
        Some(LineStatus::Restarting)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_keeps_only_the_last_6000_lines() {
        let mut buf = LogBuffer::new(MAX_LOG_LINES);
        for i in 0..6500 {
            buf.push_text(&format!("line {i}\n"));
        }
        assert_eq!(buf.len(), 6000);
        assert_eq!(buf.lines().next(), Some("line 500"));
        assert_eq!(buf.lines().last(), Some("line 6499"));
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn a_partial_line_is_completed_by_the_next_push() {
        let mut buf = LogBuffer::new(10);
        buf.push_text("ab\ncd");
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.lines().collect::<Vec<_>>(), ["ab", "cd"]);
        buf.push_text("ef\r\ngh\n");
        assert_eq!(buf.lines().collect::<Vec<_>>(), ["ab", "cdef", "gh"]);
        assert_eq!(buf.text(), "ab\ncdef\ngh");
    }

    #[test]
    fn ten_megabytes_without_newline_stay_one_line() {
        let mut buf = LogBuffer::new(MAX_LOG_LINES);
        let chunk = "x".repeat(1 << 20);
        for _ in 0..10 {
            buf.push_text(&chunk);
        }
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.lines().count(), 1);
        assert_eq!(buf.text().len(), 10 << 20);
        buf.push_text("\n");
        assert_eq!(buf.len(), 1);
        buf.push_text("next\n");
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn run_log_keeps_going_after_a_write_error() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "x").unwrap();
        let mut log = RunLog::open(&blocker.join("last_run.log"));
        assert!(!log.is_open());
        log.write("ignored\n");
        log.write("still ignored\n");
        assert!(!log.is_open());
    }

    #[test]
    fn run_log_truncates_and_creates_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("last_run.log");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "old").unwrap();
        let mut log = RunLog::open(&path);
        assert!(log.is_open());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        log.write("a\n");
        log.write("b\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nb\n");
        let deep = dir.path().join("x").join("y").join("run.log");
        assert!(RunLog::open(&deep).is_open());
    }

    #[test]
    fn hints_and_statuses_match_the_python_strings() {
        assert_eq!(
            exit_hint("mmap failed of guest memory (error 1455)"),
            Some(ExitHint::GuestMemory)
        );
        assert_eq!(
            exit_hint("guest raised debug exception code=0xa0020001 at rip"),
            Some(ExitHint::DumpIncomplete)
        );
        assert_eq!(exit_hint("just a normal line"), None);
        assert!(ExitHint::GuestMemory.text_en().contains("page file"));
        assert!(ExitHint::DumpIncomplete.text_en().contains("about 40 GB"));
        assert_eq!(
            line_status("Entering original x86-64 code"),
            Some(LineStatus::Running)
        );
        assert_eq!(
            line_status("restarting through run.py"),
            Some(LineStatus::Restarting)
        );
        assert_eq!(line_status("other"), None);
    }
}
