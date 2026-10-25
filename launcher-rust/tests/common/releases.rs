//! A release host that never reaches GitHub or the browser: it reports each call on a channel and
//! answers from a script, so the UI tests see what the window asked of the host.
#![allow(dead_code)]

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use spartanport_launcher::core::release::{Release, ReleaseId};
use spartanport_launcher::core::update::{InstallTarget, ReleaseHost};

/// What the window asked of the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    List,
    /// The package URL of the release.
    Install(String),
    OpenPage(String),
}

pub struct FakeReleases {
    installed: ReleaseId,
    target: InstallTarget,
    list: Result<Vec<Release>, String>,
    install: Result<(), String>,
    calls: Mutex<Sender<Call>>,
}

impl FakeReleases {
    /// A packaged Windows install with no releases listed, and the channel its calls arrive on.
    pub fn new() -> (FakeReleases, Receiver<Call>) {
        let (sender, receiver) = mpsc::channel();
        let fake = FakeReleases {
            installed: ReleaseId::fallback(),
            target: InstallTarget::WindowsPackage,
            list: Ok(Vec::new()),
            install: Ok(()),
            calls: Mutex::new(sender),
        };
        (fake, receiver)
    }

    pub fn installed(mut self, id: ReleaseId) -> Self {
        self.installed = id;
        self
    }

    pub fn target(mut self, target: InstallTarget) -> Self {
        self.target = target;
        self
    }

    pub fn list(mut self, answer: Result<Vec<Release>, String>) -> Self {
        self.list = answer;
        self
    }

    pub fn install(mut self, answer: Result<(), String>) -> Self {
        self.install = answer;
        self
    }

    pub fn host(self) -> Arc<dyn ReleaseHost> {
        Arc::new(self)
    }

    fn report(&self, call: Call) {
        let _ = self.calls.lock().unwrap().send(call);
    }
}

impl ReleaseHost for FakeReleases {
    fn installed(&self) -> ReleaseId {
        self.installed.clone()
    }

    fn target(&self) -> InstallTarget {
        self.target.clone()
    }

    fn list(&self) -> Result<Vec<Release>, String> {
        self.report(Call::List);
        self.list.clone()
    }

    fn install(&self, release: &Release, progress: &mut dyn FnMut(u8)) -> Result<(), String> {
        self.report(Call::Install(release.url.clone()));
        progress(100);
        self.install.clone()
    }

    fn open_page(&self, url: &str) {
        self.report(Call::OpenPage(url.to_string()));
    }
}

/// The next call, or a panic after two seconds: the host's calls run on worker threads.
pub fn next_call(calls: &Receiver<Call>) -> Call {
    calls
        .recv_timeout(Duration::from_secs(2))
        .expect("the host was not called")
}
