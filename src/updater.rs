//! Looking for and installing new versions from the project's GitHub releases.
//!
//! A release must contain a zip per platform whose name holds the build target,
//! e.g. `printzilla-v0.2.0-x86_64-pc-windows-msvc.zip`, with the program inside
//! (`printzilla.exe` on Windows, `printzilla` on macOS). The release workflow in
//! `.github/workflows/release.yml` creates exactly that.

use std::sync::mpsc::{channel, Receiver};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_REPO: &str = "Marwaeck/printzilla";

#[derive(Clone, Debug)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available { version: String, notes: String },
    Installing { version: String },
    Installed { version: String },
    Failed(String),
}

pub struct Updater {
    pub state: UpdateState,
    /// The update window is open.
    pub open: bool,
    /// Checked by itself at start (no window, only a hint when something is new).
    pub quiet: bool,
    rx: Option<Receiver<UpdateState>>,
}

impl Default for Updater {
    fn default() -> Self {
        Updater {
            state: UpdateState::Idle,
            open: false,
            quiet: false,
            rx: None,
        }
    }
}

fn split_repo(repo: &str) -> Result<(String, String), String> {
    let repo = repo.trim().trim_start_matches("https://github.com/").trim_end_matches('/').trim_end_matches(".git");
    match repo.split_once('/') {
        Some((o, n)) if !o.is_empty() && !n.is_empty() => Ok((o.to_string(), n.to_string())),
        _ => Err("Die Adresse muss die Form »Benutzer/Repository« haben.".into()),
    }
}

fn friendly(e: impl std::fmt::Display) -> String {
    let t = e.to_string();
    if t.contains("404") || t.to_lowercase().contains("not found") {
        "Auf GitHub wurde noch keine veröffentlichte Version gefunden.".into()
    } else if t.to_lowercase().contains("no asset") || t.to_lowercase().contains("asset") {
        "Die neue Version enthält keine Datei für dieses Betriebssystem.".into()
    } else if t.to_lowercase().contains("dns") || t.to_lowercase().contains("connect") || t.to_lowercase().contains("network") {
        "Keine Verbindung zu GitHub. Bitte Internetverbindung prüfen.".into()
    } else {
        format!("Update fehlgeschlagen: {t}")
    }
}

fn check(repo: &str) -> UpdateState {
    let (owner, name) = match split_repo(repo) {
        Ok(x) => x,
        Err(e) => return UpdateState::Failed(e),
    };
    let list = self_update::backends::github::ReleaseList::configure()
        .repo_owner(&owner)
        .repo_name(&name)
        .build()
        .and_then(|l| l.fetch());
    let releases = match list {
        Ok(r) => r,
        Err(e) => return UpdateState::Failed(friendly(e)),
    };
    let target = self_update::get_target();
    let newest = releases
        .into_iter()
        .filter(|r| r.assets.iter().any(|a| a.name.contains(target)))
        .find(|r| self_update::version::bump_is_greater(CURRENT_VERSION, r.version.trim_start_matches('v')).unwrap_or(false));
    match newest {
        Some(r) => UpdateState::Available {
            version: r.version.trim_start_matches('v').to_string(),
            notes: r.body.unwrap_or_default(),
        },
        None => UpdateState::UpToDate,
    }
}

fn install(repo: &str, version: &str) -> UpdateState {
    let (owner, name) = match split_repo(repo) {
        Ok(x) => x,
        Err(e) => return UpdateState::Failed(e),
    };
    let tag = format!("v{version}");
    let res = self_update::backends::github::Update::configure()
        .repo_owner(&owner)
        .repo_name(&name)
        .bin_name("printzilla")
        .target_version_tag(&tag)
        .current_version(CURRENT_VERSION)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .build()
        .and_then(|u| u.update());
    match res {
        Ok(_) => UpdateState::Installed { version: version.to_string() },
        Err(e) => UpdateState::Failed(friendly(e)),
    }
}

impl Updater {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    pub fn start_check(&mut self, ctx: &eframe::egui::Context, repo: &str, quiet: bool) {
        if self.busy() {
            return;
        }
        self.quiet = quiet;
        self.state = UpdateState::Checking;
        let (tx, rx) = channel();
        let repo = repo.to_string();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(check(&repo));
            c.request_repaint();
        });
        self.rx = Some(rx);
    }

    pub fn start_install(&mut self, ctx: &eframe::egui::Context, repo: &str, version: &str) {
        if self.busy() {
            return;
        }
        self.state = UpdateState::Installing { version: version.to_string() };
        let (tx, rx) = channel();
        let repo = repo.to_string();
        let version = version.to_string();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(install(&repo, &version));
            c.request_repaint();
        });
        self.rx = Some(rx);
    }

    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok(s) = rx.try_recv() {
                self.state = s;
                self.rx = None;
            }
        }
    }

    /// Starts the freshly installed program and asks this one to close.
    pub fn restart(ctx: &eframe::egui::Context) {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_addresses() {
        assert_eq!(split_repo("Marwaeck/printzilla").unwrap(), ("Marwaeck".into(), "printzilla".into()));
        assert_eq!(split_repo("https://github.com/a/b.git").unwrap(), ("a".into(), "b".into()));
        assert!(split_repo("nur-ein-name").is_err());
    }
}
