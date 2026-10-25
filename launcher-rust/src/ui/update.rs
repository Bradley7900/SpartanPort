//! The release list and the install of any listed release (spec 5.6).
//!
//! Progress: the download runs on a worker thread that sends percentages through an `mpsc`
//! channel kept in `State::update_progress`; the app clock (`Message::Tick`) drains it with
//! [`drain`], as it does for the game's output.

use std::path::PathBuf;
use std::sync::mpsc;

use iced::Task;

use crate::core::i18n::{tr, tr_fmt};
use crate::core::release::{newer_stable, version_tuple, Release, ReleaseId};
use crate::core::update::InstallTarget;
use crate::ui::app::{Message, State};
use crate::ui::play::{self, WORKER_LOST};
use crate::ui::tasks::{blocking, blocking_or};
use crate::ui::widgets::Modal;

/// Where the update stands.
#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    #[default]
    None,
    /// A check is running.
    Checking,
    /// The releases of this system, newest first.
    Ready(Vec<Release>),
    /// `release`'s package is downloading; the list stays.
    Downloading {
        releases: Vec<Release>,
        release: Release,
        percent: u8,
    },
    Failed(String),
}

/// Update events.
#[derive(Debug, Clone)]
pub enum UpdateMessage {
    /// A check: the Check for updates button (`manual`) or a silent one.
    Check {
        manual: bool,
    },
    /// The answer of a check; `manual` is what the check was started as.
    Checked {
        manual: bool,
        result: Result<Vec<Release>, String>,
    },
    /// A listed release was picked.
    Install(Release),
    /// The dialog's Yes to a read-only AppImage folder: open this release page.
    OpenPage(String),
    /// The dialog's Yes to the install.
    Confirmed(Release),
    Progress(u8),
    Done(Result<(), String>),
}

/// What a finished check means for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// A newer stable is listed.
    Offer,
    /// Manual check: nothing newer.
    Latest,
    Nothing,
}

/// The listed releases: those of `Ready` or `Downloading`, else none.
pub fn releases(state: &State) -> &[Release] {
    match &state.update {
        UpdateState::Ready(releases) | UpdateState::Downloading { releases, .. } => releases,
        _ => &[],
    }
}

/// The newest listed stable above the installed release.
pub fn newer(state: &State) -> Option<&Release> {
    newer_stable(releases(state), &state.installed)
}

/// Keeps `releases` as the list; a running download keeps its state.
pub fn ready(state: &mut State, releases: Vec<Release>) {
    if !matches!(state.update, UpdateState::Downloading { .. }) {
        state.update = UpdateState::Ready(releases);
    }
}

/// Whether `release`'s version is below the installed one.
pub fn is_older(release: &Release, installed: &ReleaseId) -> bool {
    version_tuple(&release.id.version) < version_tuple(&installed.version)
}

/// The install question, with a warning when `release` is older than the installed one.
pub fn confirm_text(lang: &str, release: &Release, installed: &ReleaseId) -> String {
    let mut text = tr_fmt(
        lang,
        "Install version {} now? The launcher closes, installs it and opens again. Saves and settings are kept.",
        &[&release.id.short()],
    );
    if is_older(release, installed) {
        text.push('\n');
        text.push_str(&tr(lang, "This version is older than the installed one."));
    }
    text
}

/// Python `check_update`'s `show`, over the list.
pub fn offer_decision(newer: bool, manual: bool) -> Decision {
    if newer {
        Decision::Offer
    } else if manual {
        Decision::Latest
    } else {
        Decision::Nothing
    }
}

/// What installing a release does (Python `install_update`, before the download).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallPlan {
    Blocked,
    /// Download by hand: open the release page.
    OpenPage,
    /// The AppImage's folder cannot be written; offer the page.
    ReadOnly(PathBuf),
    /// Ask, then download and install.
    Confirm,
}

/// Decides an install from the host's target.
pub fn install_decision(busy: bool, target: &InstallTarget) -> InstallPlan {
    if busy {
        return InstallPlan::Blocked;
    }
    match target {
        InstallTarget::WindowsPackage => InstallPlan::Confirm,
        InstallTarget::AppImage { writable: true, .. } => InstallPlan::Confirm,
        InstallTarget::AppImage { folder, .. } => InstallPlan::ReadOnly(folder.clone()),
        InstallTarget::WindowsSource | InstallTarget::Elsewhere => InstallPlan::OpenPage,
    }
}

fn open_page(state: &State, page: String) -> Task<Message> {
    let host = state.host.clone();
    blocking(move || host.open_page(&page)).map(|()| Message::Noop)
}

/// Applies an update event.
pub fn update(state: &mut State, message: UpdateMessage) -> Task<Message> {
    let lang = state.lang.clone();
    match message {
        UpdateMessage::Check { manual } => {
            if matches!(state.update, UpdateState::None | UpdateState::Failed(_)) {
                state.update = UpdateState::Checking;
            }
            let host = state.host.clone();
            blocking(move || host.list())
                .map(move |result| Message::Update(UpdateMessage::Checked { manual, result }))
        }
        UpdateMessage::Checked { manual, result } => {
            match result {
                Ok(releases) => {
                    ready(state, releases);
                    if offer_decision(newer(state).is_some(), manual) == Decision::Latest {
                        let text = tr_fmt(
                            &lang,
                            "You have the latest version ({}).",
                            &[&state.installed.version],
                        );
                        state.modal = Some(Modal::info(text));
                    }
                }
                Err(error) => {
                    // A list already shown and a running download stay.
                    if matches!(
                        state.update,
                        UpdateState::None | UpdateState::Checking | UpdateState::Failed(_)
                    ) {
                        state.update = UpdateState::Failed(error.clone());
                    }
                    if manual {
                        let text = tr_fmt(&lang, "Could not check for updates: {}", &[&error]);
                        state.modal = Some(Modal::error(text));
                    }
                }
            }
            Task::none()
        }
        UpdateMessage::Install(release) => {
            if matches!(state.update, UpdateState::Downloading { .. })
                || release.id.same_build(&state.installed)
            {
                return Task::none();
            }
            let plan = install_decision(state.game.is_live(), &state.host.target());
            match plan {
                InstallPlan::Blocked => {
                    let text = tr(&lang, "Close the game before updating.");
                    state.modal = Some(Modal::info(text));
                    Task::none()
                }
                InstallPlan::OpenPage => open_page(state, release.page),
                InstallPlan::ReadOnly(folder) => {
                    let text = tr_fmt(
                        &lang,
                        "The launcher cannot write to {}, so it cannot replace itself there. Open the releases page to download the new version by hand?",
                        &[&folder.display().to_string()],
                    );
                    let yes = Message::Update(UpdateMessage::OpenPage(release.page));
                    state.modal = Some(Modal::confirm(text, yes));
                    Task::none()
                }
                InstallPlan::Confirm => {
                    let text = confirm_text(&lang, &release, &state.installed);
                    let yes = Message::Update(UpdateMessage::Confirmed(release));
                    state.modal = Some(Modal::confirm(text, yes));
                    Task::none()
                }
            }
        }
        UpdateMessage::OpenPage(page) => {
            state.modal = None;
            open_page(state, page)
        }
        UpdateMessage::Confirmed(release) => {
            state.modal = None;
            // The question may have been asked before the game started.
            if play::refuse_while_live(state, play::LiveAction::Update) {
                return Task::none();
            }
            state.update = UpdateState::Downloading {
                releases: releases(state).to_vec(),
                release: release.clone(),
                percent: 0,
            };
            let (sender, receiver) = mpsc::channel();
            state.update_progress = Some(receiver);
            let host = state.host.clone();
            // A download that dies still ends `Downloading`, so the list comes back.
            blocking_or(
                move || {
                    host.install(&release, &mut |percent| {
                        let _ = sender.send(percent);
                    })
                },
                Err(WORKER_LOST.to_string()),
            )
            .map(|result| Message::Update(UpdateMessage::Done(result)))
        }
        UpdateMessage::Progress(percent) => {
            if let UpdateState::Downloading { percent: shown, .. } = &mut state.update {
                *shown = percent;
            }
            Task::none()
        }
        UpdateMessage::Done(Ok(())) => iced::exit(),
        UpdateMessage::Done(Err(error)) => {
            state.update_progress = None;
            if let UpdateState::Downloading { releases, .. } = &mut state.update {
                state.update = UpdateState::Ready(std::mem::take(releases));
            }
            let text = tr_fmt(&lang, "Update failed: {}", &[&error]);
            state.modal = Some(Modal::error(text));
            Task::none()
        }
    }
}

/// Applies the percentages the worker sent since the last clock tick.
pub fn drain(state: &mut State) {
    let Some(receiver) = &state.update_progress else {
        return;
    };
    let latest = receiver.try_iter().last();
    if let Some(percent) = latest {
        let _ = update(state, UpdateMessage::Progress(percent));
    }
}
