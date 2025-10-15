use anyhow::{Context, Result};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::types::WatchEvent;

/// Manages filesystem watchers for repositories
pub struct WatchManager {
    watchers: Arc<Mutex<HashMap<String, RepoWatcher>>>,
}

struct RepoWatcher {
    _watcher: RecommendedWatcher,
    repo_id: String,
    repo_path: PathBuf,
}

/// Debounced event collector
struct EventCollector {
    events: Arc<Mutex<HashMap<String, PendingEvent>>>,
}

struct PendingEvent {
    paths: HashSet<PathBuf>,
    event_type: String,
    last_update: Instant,
}

impl WatchManager {
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts watching a repository
    pub fn watch_repo(
        &self,
        repo_id: String,
        repo_path: PathBuf,
        app_handle: AppHandle,
    ) -> Result<()> {
        let mut watchers = self.watchers.lock().unwrap();

        // Don't create duplicate watchers
        if watchers.contains_key(&repo_id) {
            return Ok(());
        }

        let collector = EventCollector::new();
        let repo_id_clone = repo_id.clone();
        let repo_path_clone = repo_path.clone();
        let collector_clone = collector.clone();

        // Create the watcher
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    collector_clone.handle_event(event, &repo_id_clone, &repo_path_clone);
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(200)),
        )
        .context("Failed to create filesystem watcher")?;

        // Watch the repository directory
        watcher
            .watch(&repo_path, RecursiveMode::Recursive)
            .context("Failed to watch repository directory")?;

        // Start the debounce loop
        let app_handle_clone = app_handle.clone();
        let repo_id_clone2 = repo_id.clone();
        std::thread::spawn(move || {
            collector.debounce_loop(repo_id_clone2, app_handle_clone);
        });

        // Store the watcher
        watchers.insert(
            repo_id.clone(),
            RepoWatcher {
                _watcher: watcher,
                repo_id,
                repo_path,
            },
        );

        Ok(())
    }

    /// Stops watching a repository
    pub fn unwatch_repo(&self, repo_id: &str) -> Result<()> {
        let mut watchers = self.watchers.lock().unwrap();
        watchers
            .remove(repo_id)
            .ok_or_else(|| anyhow::anyhow!("Watcher not found for repo: {}", repo_id))?;
        Ok(())
    }

    /// Lists all currently watched repositories
    pub fn list_watched(&self) -> Vec<String> {
        let watchers = self.watchers.lock().unwrap();
        watchers.keys().cloned().collect()
    }
}

impl Default for WatchManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EventCollector {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn clone(&self) -> Self {
        Self {
            events: Arc::clone(&self.events),
        }
    }

    fn handle_event(&self, event: Event, repo_id: &str, repo_path: &Path) {
        // Check if this is a .git/index or .git/HEAD change (staging/branch change)
        let is_git_metadata = event.paths.iter().any(|p| {
            let rel_path = p.strip_prefix(repo_path).ok();
            rel_path.map_or(false, |rel| {
                let path_str = rel.to_string_lossy();
                path_str.starts_with(".git/index")
                    || path_str.starts_with(".git/HEAD")
                    || path_str.starts_with(".git/refs/")
                    || path_str.starts_with(".git/ORIG_HEAD")
            })
        });

        // Filter out other .git directory events (but keep index, HEAD, refs)
        let paths: Vec<PathBuf> = event
            .paths
            .into_iter()
            .filter(|p| {
                let rel_path = p.strip_prefix(repo_path).ok();
                if let Some(rel) = rel_path {
                    let path_str = rel.to_string_lossy();
                    // Allow .git/index, .git/HEAD, .git/refs, but block other .git files
                    !path_str.starts_with(".git/")
                        || path_str.starts_with(".git/index")
                        || path_str.starts_with(".git/HEAD")
                        || path_str.starts_with(".git/refs/")
                        || path_str.starts_with(".git/ORIG_HEAD")
                } else {
                    false
                }
            })
            .filter_map(|p| {
                // Convert to relative path
                p.strip_prefix(repo_path).ok().map(|rel| rel.to_path_buf())
            })
            .collect();

        if paths.is_empty() && !is_git_metadata {
            return;
        }

        // Determine event type - use "repo-changed" for git metadata
        let event_type = if is_git_metadata {
            "repo-changed"
        } else {
            match event.kind {
                EventKind::Create(_) => "created",
                EventKind::Modify(_) => "modified",
                EventKind::Remove(_) => "deleted",
                _ => "modified", // Default to modified
            }
        };

        let mut events = self.events.lock().unwrap();
        let pending = events
            .entry(repo_id.to_string())
            .or_insert_with(|| PendingEvent {
                paths: HashSet::new(),
                event_type: event_type.to_string(),
                last_update: Instant::now(),
            });

        pending.paths.extend(paths);
        pending.last_update = Instant::now();
    }

    fn debounce_loop(&self, repo_id: String, app_handle: AppHandle) {
        loop {
            std::thread::sleep(Duration::from_millis(200));

            let mut events = self.events.lock().unwrap();

            if let Some(pending) = events.get(&repo_id) {
                // Check if enough time has passed since last update
                if pending.last_update.elapsed() >= Duration::from_millis(200) {
                    // Emit the event
                    let paths: Vec<String> = pending
                        .paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();

                    let watch_event = WatchEvent {
                        repo_id: repo_id.clone(),
                        paths,
                        event_type: pending.event_type.clone(),
                    };

                    // Emit to all windows
                    let _ = app_handle.emit("watch-event", &watch_event);

                    // Clear the pending event
                    events.remove(&repo_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_watch_manager_creation() {
        let manager = WatchManager::new();
        assert_eq!(manager.list_watched().len(), 0);
    }

    // Note: Full integration tests for file watching require a Tauri app context
    // and are better suited for integration tests rather than unit tests
}
