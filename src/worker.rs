use crate::{
    ai, grouping,
    model::{Item, Layout},
    operations::{self, Plan},
    persistence, updates,
};
use anyhow::{Result, ensure};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};

pub enum Job {
    Save(u64, Layout, PathBuf),
    Scan(PathBuf),
    Import(Vec<PathBuf>, u64),
    Ai {
        base: String,
        model: String,
        key: Vec<u8>,
        items: Vec<Item>,
    },
    Preview(PathBuf, Vec<Item>, Vec<grouping::Suggestion>),
    Apply(Plan, PathBuf),
    Undo(PathBuf),
    Recover(PathBuf),
    Check,
    Download(updates::Available),
    Install(updates::Prepared),
}
pub enum Event {
    Saved(u64, Result<()>),
    Scanned(Result<Vec<(PathBuf, bool)>>),
    Imported(u64, Result<Vec<(PathBuf, bool)>>),
    Suggested(Result<Vec<grouping::Suggestion>>),
    Previewed(Result<Plan>),
    Applied(Result<Plan>),
    Undone(Result<Plan>),
    Recovered(Result<Plan>),
    Checked(Result<updates::CheckStatus>),
    Progress(u64, u64),
    Downloaded(Result<updates::Prepared>),
    Installed(Result<()>),
}

pub struct Worker {
    sender: SyncSender<Job>,
    network: SyncSender<Job>,
    pub receiver: Receiver<Event>,
    wake: Arc<AtomicUsize>,
}
impl Worker {
    pub fn start(window: usize, message: u32) -> Self {
        let (sender, jobs) = mpsc::sync_channel::<Job>(8);
        let (network, network_jobs) = mpsc::sync_channel::<Job>(4);
        let (events, receiver) = mpsc::sync_channel(128);
        let wake = Arc::new(AtomicUsize::new(window));
        for (name, jobs) in [
            ("organizer-files", jobs),
            ("organizer-network", network_jobs),
        ] {
            let worker_wake = wake.clone();
            let events = events.clone();
            thread::Builder::new()
                .name(name.into())
                .spawn(move || {
                    let publish = |event| {
                        if events.send(event).is_ok() {
                            #[cfg(windows)]
                            {
                                let hwnd = worker_wake.load(Ordering::Acquire);
                                if hwnd != 0 {
                                    unsafe {
                                        windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                                            hwnd as _, message, 0, 0,
                                        );
                                    }
                                }
                            }
                            #[cfg(not(windows))]
                            {
                                let _ = (&worker_wake, message);
                            }
                        }
                    };
                    while let Ok(job) = jobs.recv() {
                        let event = match job {
                            Job::Save(sequence, layout, path) => {
                                Event::Saved(sequence, persistence::save(&path, &layout))
                            }
                            Job::Scan(root) => Event::Scanned(scan(root)),
                            Job::Import(paths, group) => Event::Imported(group, inspect(paths)),
                            Job::Ai {
                                base,
                                model,
                                key,
                                items,
                            } => {
                                let result = (|| {
                                    #[cfg(windows)]
                                    let mut plaintext = crate::platform::secrets::unprotect(&key)?;
                                    #[cfg(not(windows))]
                                    let mut plaintext = key;
                                    let result = ai::request(
                                        &base,
                                        &model,
                                        std::str::from_utf8(&plaintext)?,
                                        &items,
                                    );
                                    plaintext.fill(0);
                                    result
                                })();
                                Event::Suggested(result)
                            }
                            Job::Preview(root, items, groups) => {
                                Event::Previewed(operations::preview(&root, &items, &groups))
                            }
                            Job::Apply(mut plan, journal) => {
                                let result = plan.apply(&journal).map(|_| plan);
                                Event::Applied(result)
                            }
                            Job::Undo(journal) => {
                                let result = persistence::load::<Plan>(&journal)
                                    .and_then(|mut p| p.undo(&journal).map(|_| p));
                                Event::Undone(result)
                            }
                            Job::Recover(journal) => {
                                let result =
                                    persistence::load::<Plan>(&journal).and_then(|mut plan| {
                                        plan.recover()?;
                                        persistence::save(&journal, &plan)?;
                                        Ok(plan)
                                    });
                                Event::Recovered(result)
                            }
                            Job::Check => Event::Checked(updates::check()),
                            Job::Download(update) => {
                                Event::Downloaded(updates::download(&update, |n, total| {
                                    publish(Event::Progress(n, total))
                                }))
                            }
                            Job::Install(prepared) => Event::Installed(updates::install(prepared)),
                        };
                        publish(event);
                    }
                })
                .expect("could not start bounded background worker");
        }
        Self {
            sender,
            network,
            receiver,
            wake,
        }
    }
    pub fn submit(&self, job: Job) -> Result<()> {
        let sender = if matches!(job, Job::Ai { .. } | Job::Check | Job::Download(..)) {
            &self.network
        } else {
            &self.sender
        };
        sender.try_send(job).map_err(|_| {
            anyhow::anyhow!("Background queue is full or unavailable; retry after the current task")
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.wake.store(0, Ordering::Release);
    }
}

fn inspect(paths: Vec<PathBuf>) -> Result<Vec<(PathBuf, bool)>> {
    ensure!(
        paths.len() <= crate::model::MAX_ITEMS,
        "Too many dropped items"
    );
    let mut entries = vec![];
    for path in paths {
        operations::checked_path(&path, true)?;
        let meta = fs::symlink_metadata(&path)?;
        ensure!(meta.is_file() || meta.is_dir(), "Unsupported item type");
        entries.push((path, meta.is_dir()));
    }
    Ok(entries)
}
fn scan(root: PathBuf) -> Result<Vec<(PathBuf, bool)>> {
    operations::checked_path(&root, true)?;
    let mut paths = vec![];
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("desktop.ini"))
        {
            continue;
        }
        paths.push(path);
        ensure!(
            paths.len() <= crate::model::MAX_ITEMS,
            "Root exceeds 2000 items. Use a smaller folder or drag selected files."
        );
    }
    paths.sort();
    inspect(paths)
}
