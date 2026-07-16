//! Index / staging operations using gix

use std::path::Path;

use gix::bstr::{BStr, BString, ByteSlice};
use gix::index::entry;

use super::{gix_err, is_executable, Git, GitError};

impl Git {
    /// Stage a single file for commit
    pub fn stage_file(&self, path: &Path) -> Result<(), GitError> {
        self.stage_files(&[path])
    }

    /// Stage multiple files for commit
    pub fn stage_files(&self, paths: &[&Path]) -> Result<(), GitError> {
        if paths.is_empty() {
            return Ok(());
        }

        let repo = self.open_repo()?;
        let mut index = (*repo.index_or_empty().map_err(gix_err)?).clone();
        let index_mut = &mut index;

        for path in paths {
            // Make path relative to repo root
            let rel_path = match path.strip_prefix(&self.repo_root) {
                Ok(rel) => rel,
                Err(_) => path,
            };
            // Git index entries must use '/' separators; on Windows,
            // to_string_lossy yields backslashes, which would create
            // duplicate entries and corrupt tree objects.
            let rel_str: BString = rel_path.to_string_lossy().replace('\\', "/").into();

            let full_path = self.repo_root.join(rel_path);

            if full_path.exists() {
                // Add or update the file in the index
                let content = std::fs::read(&full_path)?;
                let blob_id = repo.write_blob(&content).map_err(gix_err)?;

                let metadata = std::fs::metadata(&full_path)?;
                let mode = if is_executable(&metadata) {
                    entry::Mode::FILE_EXECUTABLE
                } else {
                    entry::Mode::FILE
                };

                // Remove existing entry for this path if present
                let rel_bstr: &BStr = rel_str.as_bstr();
                index_mut.remove_entries(|_idx, entry_path, _entry| entry_path == rel_bstr);

                // Push new entry
                index_mut.dangerously_push_entry(
                    entry::Stat::default(),
                    blob_id.detach(),
                    entry::Flags::empty(),
                    mode,
                    rel_bstr,
                );
            } else {
                // File was deleted — remove from index
                let rel_bstr: &BStr = rel_str.as_bstr();
                index_mut.remove_entries(|_idx, entry_path, _entry| entry_path == rel_bstr);
            }
        }

        // Sort entries to maintain index invariants
        index_mut.sort_entries();

        // Write the index back
        index
            .write(gix::index::write::Options {
                extensions: Default::default(),
                skip_hash: false,
            })
            .map_err(gix_err)?;

        Ok(())
    }

    /// Get list of staged files (files in index that differ from HEAD)
    pub fn staged_files(&self) -> Result<Vec<String>, GitError> {
        let repo = self.open_repo()?;
        let index = repo.index_or_empty().map_err(gix_err)?;

        // Get HEAD tree (if it exists)
        let head_tree = match repo.head_id() {
            Ok(head_id) => {
                let commit = repo.find_commit(head_id.detach()).map_err(gix_err)?;
                Some(commit.tree().map_err(gix_err)?)
            }
            Err(_) => None, // No commits yet — everything in index is "staged"
        };

        let mut staged = Vec::new();

        for entry in index.entries() {
            let path = entry.path_in(index.path_backing());
            let path_str = path.to_str_lossy().to_string();

            if let Some(ref tree) = head_tree {
                // Check if this entry differs from HEAD
                match tree.lookup_entry_by_path(path_str.as_ref() as &std::path::Path) {
                    Ok(Some(tree_entry)) => {
                        if tree_entry.object_id() != entry.id {
                            staged.push(path_str);
                        }
                    }
                    Ok(None) => {
                        // File is in index but not in HEAD — it's new/added
                        staged.push(path_str);
                    }
                    Err(_) => {}
                }
            } else {
                // No HEAD commit — all index entries are staged
                staged.push(path_str);
            }
        }

        Ok(staged)
    }

    /// Check if a file path is tracked (exists in the index)
    pub fn is_tracked(&self, path: &str) -> Result<bool, GitError> {
        let repo = self.open_repo()?;
        let index = repo.index_or_empty().map_err(gix_err)?;

        // Index entries are '/'-separated; normalize Windows-style input.
        let path = path.replace('\\', "/");
        let path_bstr: &BStr = path.as_bytes().as_ref();
        for entry in index.entries() {
            let entry_path = entry.path_in(index.path_backing());
            if entry_path == path_bstr {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Unstage files (reset index entries to match HEAD)
    pub fn unstage_files(&self, paths: &[&str]) -> Result<(), GitError> {
        if paths.is_empty() {
            return Ok(());
        }

        let repo = self.open_repo()?;
        let mut index = (*repo.index_or_empty().map_err(gix_err)?).clone();

        // Get HEAD tree
        let head_tree = match repo.head_id() {
            Ok(head_id) => {
                let commit = repo.find_commit(head_id.detach()).map_err(gix_err)?;
                Some(commit.tree().map_err(gix_err)?)
            }
            Err(_) => None,
        };

        for path in paths {
            // Index entries are '/'-separated; normalize Windows-style input.
            let path = path.replace('\\', "/");
            let path_bstr: &BStr = path.as_bytes().as_ref();

            if let Some(ref tree) = head_tree {
                match tree.lookup_entry_by_path(path.as_ref() as &std::path::Path) {
                    Ok(Some(tree_entry)) => {
                        // File exists in HEAD — restore the index entry from HEAD
                        index.remove_entries(|_idx, entry_path, _entry| entry_path == path_bstr);
                        index.dangerously_push_entry(
                            entry::Stat::default(),
                            tree_entry.object_id(),
                            entry::Flags::empty(),
                            tree_entry.mode().into(),
                            path_bstr,
                        );
                    }
                    Ok(None) => {
                        // File doesn't exist in HEAD — remove from index entirely
                        index.remove_entries(|_idx, entry_path, _entry| entry_path == path_bstr);
                    }
                    Err(_) => {}
                }
            } else {
                // No HEAD — remove from index
                index.remove_entries(|_idx, entry_path, _entry| entry_path == path_bstr);
            }
        }

        index.sort_entries();
        index
            .write(gix::index::write::Options {
                extensions: Default::default(),
                skip_hash: false,
            })
            .map_err(gix_err)?;

        Ok(())
    }
}
