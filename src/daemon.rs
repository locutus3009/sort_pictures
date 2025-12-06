//! Daemon mode with file system watching for sort_pictures.
//!
//! Spawns watcher threads for each configured directory,
//! processes new files as they appear.

use notify::event::{CreateKind, ModifyKind, RenameMode};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use crate::config::Dir;
use crate::files;

/// Configuration for daemon behavior.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Whether to log skip messages for filtered files.
    pub log_skipped: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self { log_skipped: true }
    }
}

/// Handle for controlling a directory watcher thread.
pub struct WatchHandle {
    /// Thread join handle.
    thread: JoinHandle<()>,
    /// Path being watched.
    #[allow(dead_code)]
    path: PathBuf,
}

impl WatchHandle {
    /// Wait for the watcher thread to finish.
    ///
    /// Note: Watcher threads run indefinitely until the process exits.
    pub fn join(self) {
        let _ = self.thread.join();
    }
}

/// Starts watching a single directory for new files.
///
/// Creates a watcher thread that monitors the directory and calls the processor
/// function for each new or renamed file.
///
/// # Arguments
/// * `dir` - Directory configuration to watch
/// * `processor` - Function to call for each new file
/// * `daemon_config` - Daemon behavior configuration
///
/// # Returns
/// `WatchHandle` for the spawned watcher thread.
pub fn watch_directory<F>(dir: &Dir, processor: F, daemon_config: &DaemonConfig) -> WatchHandle
where
    F: Fn(&Path) + Send + 'static,
{
    let path = dir.source.clone().expect("Dir must have source path");
    let log_skipped = daemon_config.log_skipped;

    let thread = thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<Result<Event, notify::Error>>();

        println!("Watch: \"{}\"", path.display());

        let mut watcher = notify::recommended_watcher(tx).expect("Failed to create watcher");

        watcher
            .watch(
                &path.canonicalize().expect("Failed to canonicalize path"),
                RecursiveMode::NonRecursive,
            )
            .expect("Failed to start watching");

        for res in rx {
            match res {
                Ok(event) => {
                    if let Some(file_path) = should_process_event(&event) {
                        if let Some(reason) = files::should_skip_file(file_path) {
                            if log_skipped {
                                println!("Skip: \"{}\" ({:?})", file_path.display(), reason);
                            }
                            continue;
                        }

                        processor(file_path);
                    }
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }
    });

    WatchHandle {
        thread,
        path: dir.source.clone().unwrap(),
    }
}

/// Determines if an event should trigger file processing.
///
/// Returns the path to process if the event is a file creation or rename-to.
fn should_process_event(event: &Event) -> Option<&Path> {
    match event.kind {
        EventKind::Create(CreateKind::File) => event.paths.first().map(|p| p.as_path()),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            event.paths.first().map(|p| p.as_path())
        }
        _ => None,
    }
}

/// Runs the daemon with file watching for all configured directories.
///
/// Spawns a watcher thread for each directory and blocks until all threads
/// complete (which only happens on process termination).
///
/// # Arguments
/// * `dirs` - Slice of directory configurations to watch
/// * `processor_factory` - Function that creates a processor for each directory
///
/// # Type Parameters
/// * `F` - Processor function type
/// * `G` - Factory function that takes a Dir reference and returns a processor
pub fn run_daemon<F, G>(dirs: &[Dir], processor_factory: G, daemon_config: DaemonConfig)
where
    F: Fn(&Path) + Send + 'static,
    G: Fn(&Dir) -> F,
{
    let handles: Vec<_> = dirs
        .iter()
        .filter(|dir| dir.source.is_some())
        .map(|dir| {
            let processor = processor_factory(dir);
            watch_directory(dir, processor, &daemon_config)
        })
        .collect();

    // Wait for all watchers (they run indefinitely)
    for handle in handles {
        handle.join();
    }
}

/// Simplified daemon runner that uses a single processor for all directories.
///
/// # Arguments
/// * `dirs` - Slice of directory configurations to watch
/// * `processor` - Processor function to call for each file
pub fn run_daemon_simple<F>(dirs: &[Dir], processor: F, daemon_config: DaemonConfig)
where
    F: Fn(&Path) + Send + Clone + 'static,
{
    run_daemon(dirs, |_| processor.clone(), daemon_config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_process_event_create_file() {
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![PathBuf::from("/test/file.jpg")],
            attrs: Default::default(),
        };

        let result = should_process_event(&event);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Path::new("/test/file.jpg"));
    }

    #[test]
    fn test_should_process_event_rename_to() {
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            paths: vec![PathBuf::from("/test/file.jpg")],
            attrs: Default::default(),
        };

        let result = should_process_event(&event);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Path::new("/test/file.jpg"));
    }

    #[test]
    fn test_should_process_event_other_events() {
        // Create directory event should be ignored
        let event = Event {
            kind: EventKind::Create(CreateKind::Folder),
            paths: vec![PathBuf::from("/test/dir")],
            attrs: Default::default(),
        };
        assert!(should_process_event(&event).is_none());

        // Rename from event should be ignored
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            paths: vec![PathBuf::from("/test/file.jpg")],
            attrs: Default::default(),
        };
        assert!(should_process_event(&event).is_none());

        // Access event should be ignored
        let event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Read),
            paths: vec![PathBuf::from("/test/file.jpg")],
            attrs: Default::default(),
        };
        assert!(should_process_event(&event).is_none());
    }

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert!(config.log_skipped);
    }
}
