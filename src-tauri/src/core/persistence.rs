use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Version for workspace store schema
const WORKSPACE_VERSION: u32 = 1;

/// Version for settings store schema
const SETTINGS_VERSION: u32 = 1;

/// Workspace state that persists across app restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    pub version: u32,
    pub repos: Vec<RepoEntry>,
    pub active_repo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub path: String,
    #[serde(default)]
    pub last_opened: Option<i64>, // Unix timestamp
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            version: WORKSPACE_VERSION,
            repos: Vec::new(),
            active_repo_path: None,
        }
    }
}

/// App settings that persist across app restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsState {
    pub version: u32,
    pub theme: String, // "light" | "dark" | "system"
    pub show_line_numbers: bool,
    pub context_lines: ContextLines,
    pub compact_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ContextLines {
    Number(u32),
    All(String), // "all"
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            theme: "system".to_string(),
            show_line_numbers: false,
            context_lines: ContextLines::Number(3),
            compact_mode: false,
        }
    }
}

impl WorkspaceState {
    /// Migrate from older versions if needed
    pub fn migrate(mut self) -> Self {
        if self.version < WORKSPACE_VERSION {
            // Apply migrations here as schema evolves
            self.version = WORKSPACE_VERSION;
        }
        self
    }

    /// Validate repo paths exist and are accessible
    pub fn validate_repos(&mut self) -> Vec<String> {
        let mut missing_paths = Vec::new();

        self.repos.retain(|repo| {
            let path = PathBuf::from(&repo.path);
            let exists = path.exists() && path.join(".git").exists();
            if !exists {
                missing_paths.push(repo.path.clone());
            }
            exists
        });

        // Clear active repo if it's missing
        if let Some(active_path) = &self.active_repo_path {
            if !self.repos.iter().any(|r| &r.path == active_path) {
                self.active_repo_path = None;
            }
        }

        missing_paths
    }
}

impl SettingsState {
    /// Migrate from older versions if needed
    pub fn migrate(mut self) -> Self {
        if self.version < SETTINGS_VERSION {
            // Apply migrations here as schema evolves
            self.version = SETTINGS_VERSION;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, String) {
        use git2::Repository;

        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_string_lossy().to_string();

        let repo = Repository::init(&repo_path).unwrap();
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();

        // Create initial commit
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        )
        .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_workspace_default() {
        let workspace = WorkspaceState::default();
        assert_eq!(workspace.version, WORKSPACE_VERSION);
        assert_eq!(workspace.repos.len(), 0);
        assert_eq!(workspace.active_repo_path, None);
    }

    #[test]
    fn test_settings_default() {
        let settings = SettingsState::default();
        assert_eq!(settings.version, SETTINGS_VERSION);
        assert_eq!(settings.theme, "system");
        assert_eq!(settings.show_line_numbers, false);
        assert_eq!(settings.compact_mode, false);
    }

    #[test]
    fn test_workspace_serialization() {
        let workspace = WorkspaceState {
            version: 1,
            repos: vec![
                RepoEntry {
                    path: "/path/to/repo1".to_string(),
                    last_opened: Some(1234567890),
                },
                RepoEntry {
                    path: "/path/to/repo2".to_string(),
                    last_opened: None,
                },
            ],
            active_repo_path: Some("/path/to/repo1".to_string()),
        };

        let json = serde_json::to_string(&workspace).unwrap();
        let deserialized: WorkspaceState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, workspace.version);
        assert_eq!(deserialized.repos.len(), workspace.repos.len());
        assert_eq!(deserialized.active_repo_path, workspace.active_repo_path);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = SettingsState {
            version: 1,
            theme: "dark".to_string(),
            show_line_numbers: true,
            context_lines: ContextLines::Number(5),
            compact_mode: true,
        };

        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: SettingsState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.theme, settings.theme);
        assert_eq!(deserialized.show_line_numbers, settings.show_line_numbers);
        assert_eq!(deserialized.compact_mode, settings.compact_mode);
    }

    #[test]
    fn test_context_lines_number_serialization() {
        let settings = SettingsState {
            version: 1,
            theme: "light".to_string(),
            show_line_numbers: false,
            context_lines: ContextLines::Number(10),
            compact_mode: false,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"contextLines\":10"));

        let deserialized: SettingsState = serde_json::from_str(&json).unwrap();
        match deserialized.context_lines {
            ContextLines::Number(n) => assert_eq!(n, 10),
            _ => panic!("Expected ContextLines::Number"),
        }
    }

    #[test]
    fn test_context_lines_all_serialization() {
        let settings = SettingsState {
            version: 1,
            theme: "light".to_string(),
            show_line_numbers: false,
            context_lines: ContextLines::All("all".to_string()),
            compact_mode: false,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"contextLines\":\"all\""));

        let deserialized: SettingsState = serde_json::from_str(&json).unwrap();
        match deserialized.context_lines {
            ContextLines::All(s) => assert_eq!(s, "all"),
            _ => panic!("Expected ContextLines::All"),
        }
    }

    #[test]
    fn test_workspace_validate_repos_removes_missing() {
        let (_temp_dir, valid_path) = create_test_repo();

        let mut workspace = WorkspaceState {
            version: 1,
            repos: vec![
                RepoEntry {
                    path: valid_path.clone(),
                    last_opened: Some(1234567890),
                },
                RepoEntry {
                    path: "/path/that/does/not/exist".to_string(),
                    last_opened: Some(1234567890),
                },
            ],
            active_repo_path: Some(valid_path.clone()),
        };

        let missing = workspace.validate_repos();

        // Should return one missing path
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "/path/that/does/not/exist");

        // Should retain only valid repo
        assert_eq!(workspace.repos.len(), 1);
        assert_eq!(workspace.repos[0].path, valid_path);

        // Active repo should still be valid
        assert_eq!(workspace.active_repo_path, Some(valid_path));
    }

    #[test]
    fn test_workspace_validate_repos_clears_active_if_missing() {
        let (_temp_dir, valid_path) = create_test_repo();

        let mut workspace = WorkspaceState {
            version: 1,
            repos: vec![
                RepoEntry {
                    path: valid_path.clone(),
                    last_opened: Some(1234567890),
                },
                RepoEntry {
                    path: "/missing/repo".to_string(),
                    last_opened: Some(1234567890),
                },
            ],
            active_repo_path: Some("/missing/repo".to_string()),
        };

        workspace.validate_repos();

        // Should retain only valid repo
        assert_eq!(workspace.repos.len(), 1);

        // Active repo should be cleared since it was pointing to missing repo
        assert_eq!(workspace.active_repo_path, None);
    }

    #[test]
    fn test_workspace_validate_repos_all_valid() {
        let (_temp_dir1, path1) = create_test_repo();
        let (_temp_dir2, path2) = create_test_repo();

        let mut workspace = WorkspaceState {
            version: 1,
            repos: vec![
                RepoEntry {
                    path: path1.clone(),
                    last_opened: Some(1234567890),
                },
                RepoEntry {
                    path: path2.clone(),
                    last_opened: Some(1234567891),
                },
            ],
            active_repo_path: Some(path1.clone()),
        };

        let missing = workspace.validate_repos();

        // No missing repos
        assert_eq!(missing.len(), 0);

        // All repos retained
        assert_eq!(workspace.repos.len(), 2);

        // Active repo unchanged
        assert_eq!(workspace.active_repo_path, Some(path1));
    }

    #[test]
    fn test_workspace_validate_repos_all_missing() {
        let mut workspace = WorkspaceState {
            version: 1,
            repos: vec![
                RepoEntry {
                    path: "/missing/repo1".to_string(),
                    last_opened: Some(1234567890),
                },
                RepoEntry {
                    path: "/missing/repo2".to_string(),
                    last_opened: Some(1234567891),
                },
            ],
            active_repo_path: Some("/missing/repo1".to_string()),
        };

        let missing = workspace.validate_repos();

        // Should return all missing paths
        assert_eq!(missing.len(), 2);

        // All repos removed
        assert_eq!(workspace.repos.len(), 0);

        // Active repo cleared
        assert_eq!(workspace.active_repo_path, None);
    }

    #[test]
    fn test_workspace_with_duplicate_paths() {
        let workspace = WorkspaceState {
            version: 1,
            repos: vec![
                RepoEntry {
                    path: "/path/to/repo".to_string(),
                    last_opened: Some(1234567890),
                },
                RepoEntry {
                    path: "/path/to/repo".to_string(),
                    last_opened: Some(1234567891),
                },
                RepoEntry {
                    path: "/path/to/other".to_string(),
                    last_opened: Some(1234567892),
                },
            ],
            active_repo_path: Some("/path/to/repo".to_string()),
        };

        // Workspace should serialize and deserialize duplicates correctly
        let json = serde_json::to_string(&workspace).unwrap();
        let deserialized: WorkspaceState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.repos.len(), 3);
        // Both duplicates should be preserved in serialization
        assert_eq!(deserialized.repos[0].path, "/path/to/repo");
        assert_eq!(deserialized.repos[1].path, "/path/to/repo");
    }

    #[test]
    fn test_workspace_migration_no_op_for_current_version() {
        let workspace = WorkspaceState {
            version: WORKSPACE_VERSION,
            repos: vec![],
            active_repo_path: None,
        };

        let migrated = workspace.migrate();
        assert_eq!(migrated.version, WORKSPACE_VERSION);
    }

    #[test]
    fn test_workspace_migration_from_older_version() {
        let workspace = WorkspaceState {
            version: 0, // Old version
            repos: vec![],
            active_repo_path: None,
        };

        let migrated = workspace.migrate();
        assert_eq!(migrated.version, WORKSPACE_VERSION);
    }

    #[test]
    fn test_settings_migration_no_op_for_current_version() {
        let settings = SettingsState::default();
        let migrated = settings.migrate();
        assert_eq!(migrated.version, SETTINGS_VERSION);
    }

    #[test]
    fn test_settings_migration_from_older_version() {
        let settings = SettingsState {
            version: 0, // Old version
            theme: "dark".to_string(),
            show_line_numbers: true,
            context_lines: ContextLines::Number(5),
            compact_mode: false,
        };

        let migrated = settings.migrate();
        assert_eq!(migrated.version, SETTINGS_VERSION);
        // Settings should be preserved
        assert_eq!(migrated.theme, "dark");
        assert_eq!(migrated.show_line_numbers, true);
    }

    #[test]
    fn test_workspace_deserialization_with_corrupted_json() {
        // Missing required fields
        let corrupted_json = r#"{"version": 1}"#;
        let result = serde_json::from_str::<WorkspaceState>(corrupted_json);
        assert!(result.is_err());

        // Invalid JSON
        let corrupted_json = r#"{"version": 1, "repos": [broken"#;
        let result = serde_json::from_str::<WorkspaceState>(corrupted_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_settings_deserialization_with_corrupted_json() {
        // Missing required fields
        let corrupted_json = r#"{"version": 1, "theme": "dark"}"#;
        let result = serde_json::from_str::<SettingsState>(corrupted_json);
        assert!(result.is_err());

        // Invalid JSON
        let corrupted_json = r#"{"version": 1, broken"#;
        let result = serde_json::from_str::<SettingsState>(corrupted_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_workspace_with_empty_repos() {
        let workspace = WorkspaceState {
            version: 1,
            repos: vec![],
            active_repo_path: None,
        };

        let json = serde_json::to_string(&workspace).unwrap();
        let deserialized: WorkspaceState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.repos.len(), 0);
        assert_eq!(deserialized.active_repo_path, None);
    }

    #[test]
    fn test_repo_entry_without_last_opened() {
        let json = r#"{"path": "/path/to/repo"}"#;
        let entry: RepoEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.path, "/path/to/repo");
        assert_eq!(entry.last_opened, None);
    }

    #[test]
    fn test_repo_entry_with_last_opened() {
        let json = r#"{"path": "/path/to/repo", "lastOpened": 1234567890}"#;
        let entry: RepoEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.path, "/path/to/repo");
        assert_eq!(entry.last_opened, Some(1234567890));
    }

    #[test]
    fn test_workspace_validates_non_git_directory() {
        // Create a regular directory (not a git repo)
        let temp_dir = TempDir::new().unwrap();
        let non_git_path = temp_dir.path().to_string_lossy().to_string();

        let mut workspace = WorkspaceState {
            version: 1,
            repos: vec![RepoEntry {
                path: non_git_path.clone(),
                last_opened: Some(1234567890),
            }],
            active_repo_path: Some(non_git_path),
        };

        let missing = workspace.validate_repos();

        // Should detect that directory exists but is not a git repo
        assert_eq!(missing.len(), 1);
        assert_eq!(workspace.repos.len(), 0);
    }
}
