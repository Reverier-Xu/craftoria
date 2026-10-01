//! Workspace scanning: file-system layout to tree/table models.
//!
//! Pure, synchronous, and testable — no gpui state in here. The tree is
//! loaded **lazily**: every expansion scans only the current level plus the
//! immediately adjacent one, the adjacent pass existing solely to mark
//! empty directories (they render as leaves instead of stubs that collapse
//! into nothing). Dotfiles and well-known VCS/build/dependency directories
//! are skipped everywhere.

use std::path::Path;

use woocraft::{IconName, TreeItem};

/// Directory names never listed or descended into: VCS metadata, build
/// output, and vendored dependencies.
pub(crate) const IGNORED_DIRS: [&str; 3] = [".git", "target", "node_modules"];

/// Suffix marking a folder whose children have not been loaded yet.
const PENDING_SUFFIX: &str = "/__pending__";

/// One row of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryRow {
  /// The file name.
  pub name: String,
  /// Whether the entry is a directory.
  pub is_dir: bool,
  /// The size in bytes; `0` for directories.
  pub size: u64,
}

/// The pending placeholder child of a not-yet-expanded folder: a loading
/// row that keeps the folder's expand affordance alive (`is_folder` is
/// derived from having children) until the real children replace it.
pub(crate) fn pending_child(parent_id: &str) -> TreeItem {
  TreeItem::new(format!("{parent_id}{PENDING_SUFFIX}"), "").loading(true)
}

/// Whether `item` is a folder whose children have not been loaded yet.
pub(crate) fn is_pending(item: &TreeItem) -> bool {
  item.children.len() == 1 && item.children[0].id.ends_with(PENDING_SUFFIX)
}

/// The id of `name` below `prefix`; root-level ids start with `/` so the
/// tree ids double as workspace-relative paths.
fn child_id(prefix: &str, name: &str) -> String {
  if prefix.is_empty() {
    format!("/{name}")
  } else {
    format!("{prefix}/{name}")
  }
}

/// Builds one level of tree items for `dir` — the lazy loading step.
///
/// Each subdirectory is peeked into with a single adjacent-level listing
/// purely to decide emptiness: empty folders become leaves (no expand
/// affordance), non-empty ones keep a [`pending_child`] that is replaced
/// when they are expanded themselves. The peek results are not cached;
/// expanding a folder re-lists it, which is the price of always-fresh
/// listings and stays at one `read_dir` per displayed entry.
pub(crate) fn dir_items(dir: &Path, prefix: &str) -> Vec<TreeItem> {
  list_dir(dir)
    .into_iter()
    .map(|row| {
      let id = child_id(prefix, &row.name);
      let mut item = TreeItem::new(id.clone(), row.name.clone()).icon(icon_for(&row));
      if row.is_dir && !list_dir(&dir.join(&row.name)).is_empty() {
        item = item.child(pending_child(&id));
      }
      item
    })
    .collect()
}

/// The workspace-relative path of a tree id.
pub(crate) fn relative_path(id: &str) -> &str {
  id.trim_start_matches('/')
}

/// Lists one directory non-recursively: dotfiles and [`IGNORED_DIRS`]
/// skipped, folders first, then names.
///
/// Unreadable directories list as empty rather than failing the whole scan.
pub(crate) fn list_dir(dir: &Path) -> Vec<EntryRow> {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(_) => return Vec::new(),
  };

  let mut rows = entries
    .flatten()
    .filter_map(|entry| {
      let name = entry.file_name().to_string_lossy().into_owned();
      if name.starts_with('.') {
        return None;
      }
      let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
      if is_dir && IGNORED_DIRS.contains(&name.as_str()) {
        return None;
      }
      let size = entry.metadata().map_or(0, |meta| meta.len());
      Some(EntryRow { name, is_dir, size })
    })
    .collect::<Vec<_>>();

  rows.sort_by(|a, b| match (a.is_dir, b.is_dir) {
    (true, false) => std::cmp::Ordering::Less,
    (false, true) => std::cmp::Ordering::Greater,
    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
  });
  rows
}

/// The tree icon for a listing row: folders, source files, documents, and a
/// generic file for everything else.
fn icon_for(row: &EntryRow) -> IconName {
  if row.is_dir {
    return IconName::Folder;
  }

  match row.name.rsplit_once('.').map(|(_, ext)| ext) {
    Some("rs") => IconName::Code,
    Some("md") => IconName::DocumentText,
    _ => IconName::Document,
  }
}

/// Formats a byte size for the listing's Size column.
pub(crate) fn format_size(size: u64) -> String {
  const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
  let mut value = size as f64;
  let mut unit = 0;
  while value >= 1024. && unit < UNITS.len() - 1 {
    value /= 1024.;
    unit += 1;
  }
  if unit == 0 {
    format!("{size} B")
  } else {
    format!("{value:.1} {}", UNITS[unit])
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Creates a fixture directory tree under the system temp dir and hands
  /// back its path; the caller is responsible for removal.
  fn fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("craftoria-explorer-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/nested")).unwrap();
    std::fs::create_dir_all(root.join("empty")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("Cargo.toml"), "").unwrap();
    std::fs::write(root.join("README.md"), "").unwrap();
    std::fs::write(root.join("src/main.rs"), "").unwrap();
    std::fs::write(root.join("src/nested/mod.rs"), "").unwrap();
    root
  }

  #[test]
  fn list_dir_skips_ignored_and_sorts_folders_first() {
    let root = fixture("list");
    let rows = list_dir(&root);

    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, vec!["empty", "src", "Cargo.toml", "README.md"]);
    assert!(rows[0].is_dir && rows[1].is_dir);

    std::fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn dir_items_lazily_marks_empty_and_pending_folders() {
    let root = fixture("lazy");

    let items = dir_items(&root, "");
    let by_name = |name: &str| {
      items
        .iter()
        .find(|item| item.label.as_ref() == name)
        .unwrap_or_else(|| panic!("missing {name}"))
    };

    // `src` has entries, so it keeps a pending placeholder child …
    let src = by_name("src");
    assert_eq!(src.id.as_ref(), "/src");
    assert!(is_pending(src), "non-empty folders stay pending");
    assert!(
      src.children[0].is_loading(),
      "the placeholder is a loading row"
    );

    // … while `empty` renders as a leaf: no affordance, no stub.
    let empty = by_name("empty");
    assert_eq!(empty.children.len(), 0);
    assert!(!is_pending(empty));

    // Files never carry children.
    assert_eq!(by_name("Cargo.toml").children.len(), 0);

    // Expanding one level: children ids are workspace-relative paths and
    // non-empty grandchildren go pending again.
    let children = dir_items(&root.join("src"), "/src");
    let names: Vec<&str> = children.iter().map(|item| item.label.as_ref()).collect();
    assert_eq!(names, vec!["nested", "main.rs"]);
    assert_eq!(children[0].id.as_ref(), "/src/nested");
    assert!(is_pending(&children[0]));

    std::fs::remove_dir_all(&root).unwrap();
  }

  #[test]
  fn relative_path_strips_the_root_slash() {
    assert_eq!(relative_path("/src/nested"), "src/nested");
    assert_eq!(relative_path("/Cargo.toml"), "Cargo.toml");
  }

  #[test]
  fn sizes_format_compactly() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(42), "42 B");
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(1024 * 1024 * 3), "3.0 MB");
  }
}
