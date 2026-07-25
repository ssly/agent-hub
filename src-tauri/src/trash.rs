use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::join_relative;

const RETENTION_SECS: u64 = 7 * 24 * 3600;

fn trash_dir() -> PathBuf {
    join_relative(
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
        ".agent-hub/trash",
    )
}

fn skills_dir() -> PathBuf {
    trash_dir().join("skills")
}

fn index_path() -> PathBuf {
    trash_dir().join("index.json")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn remove_path_any(path: &std::path::Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn make_id() -> String {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", t)
}

// --- Data types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrashItemType {
    Skill,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashItem {
    pub id: String,
    pub item_type: TrashItemType,
    pub name: String,
    pub platform_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_config: Option<serde_json::Value>,
    pub deleted_at: u64,
}

// --- Index management ---

fn read_index() -> Vec<TrashItem> {
    let path = index_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let items: Vec<TrashItem> = serde_json::from_str(&content).unwrap_or_default();
    // Clean expired items
    let cutoff = now_secs().saturating_sub(RETENTION_SECS);
    let (valid, expired): (Vec<_>, Vec<_>) =
        items.into_iter().partition(|item| item.deleted_at > cutoff);
    for item in &expired {
        if matches!(item.item_type, TrashItemType::Skill) {
            let skill_path = skills_dir().join(&item.id);
            if skill_path.exists() {
                let _ = remove_path_any(&skill_path);
            }
        }
    }
    if !expired.is_empty() {
        let content = serde_json::to_string_pretty(&valid).unwrap_or_default();
        let _ = fs::write(&path, content);
    }
    valid
}

fn save_index(items: &[TrashItem]) -> Result<(), String> {
    let path = index_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(items).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())
}

fn add_item(item: TrashItem) -> Result<(), String> {
    let mut items = read_index();
    items.push(item);
    save_index(&items)
}

fn remove_item(id: &str) -> Result<TrashItem, String> {
    let mut items = read_index();
    let pos = items
        .iter()
        .position(|i| i.id == id)
        .ok_or_else(|| format!("Item {} not found in trash", id))?;
    let item = items.remove(pos);
    save_index(&items)?;
    Ok(item)
}

// --- Move to trash ---

pub fn move_skill_to_trash(
    platform_id: &str,
    name: &str,
    folder: &str,
    original_path: &std::path::Path,
) -> Result<(), String> {
    let id = make_id();
    let dest = skills_dir().join(&id);
    fs::create_dir_all(dest.parent().ok_or("Cannot create skills trash dir")?)
        .map_err(|e| e.to_string())?;

    if original_path.is_symlink() {
        // Read the symlink target, then remove it
        let target = fs::read_link(original_path).map_err(|e| format!("read_link: {}", e))?;
        fs::remove_file(original_path).map_err(|e| format!("remove symlink: {}", e))?;
        // Store the symlink target as the "backup" — we recreate the symlink on restore
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        fs::write(dest.join(".symlink_target"), target.display().to_string())
            .map_err(|e| e.to_string())?;
    } else {
        fs::rename(original_path, &dest).map_err(|e| format!("rename to trash: {}", e))?;
    }

    let item = TrashItem {
        id,
        item_type: TrashItemType::Skill,
        name: name.to_string(),
        platform_id: platform_id.to_string(),
        folder: if folder.is_empty() {
            None
        } else {
            Some(folder.to_string())
        },
        original_path: Some(original_path.display().to_string()),
        original_config: None,
        deleted_at: now_secs(),
    };
    add_item(item)
}

pub fn move_mcp_to_trash(
    platform_id: &str,
    name: &str,
    config: serde_json::Value,
) -> Result<(), String> {
    let id = make_id();
    let item = TrashItem {
        id,
        item_type: TrashItemType::Mcp,
        name: name.to_string(),
        platform_id: platform_id.to_string(),
        folder: None,
        original_path: None,
        original_config: Some(config),
        deleted_at: now_secs(),
    };
    add_item(item)
}

// --- Restore ---

pub fn restore_item(id: &str) -> Result<TrashItem, String> {
    let item = remove_item(id)?;

    match item.item_type {
        TrashItemType::Skill => {
            let trash_skill_dir = skills_dir().join(&item.id);
            let original_path = item
                .original_path
                .as_ref()
                .ok_or("Missing original_path for skill")?;
            let orig = PathBuf::from(original_path);

            if orig.exists() {
                // Put the item back in index before returning error
                let mut items = read_index();
                items.push(item.clone());
                let _ = save_index(&items);
                return Err(format!("Original path already exists: {}", original_path));
            }

            if let Some(parent) = orig.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            // Check if it was a symlink
            let symlink_target_file = trash_skill_dir.join(".symlink_target");
            if symlink_target_file.exists() {
                let target = fs::read_to_string(&symlink_target_file).map_err(|e| e.to_string())?;
                let target = target.trim();
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, &orig).map_err(|e| format!("symlink: {}", e))?;
                #[cfg(windows)]
                std::os::windows::fs::symlink_file(target, &orig)
                    .map_err(|e| format!("symlink: {}", e))?;
                // Clean up the trash dir
                let _ = fs::remove_dir_all(&trash_skill_dir);
            } else {
                fs::rename(&trash_skill_dir, &orig)
                    .map_err(|e| format!("restore rename: {}", e))?;
            }
        }
        TrashItemType::Mcp => {
            let config = item
                .original_config
                .as_ref()
                .ok_or("Missing original_config for MCP")?;
            crate::mcp::save_mcp_server(&item.platform_id, &item.name, config.clone())?;
        }
    }

    Ok(item)
}

// --- Permanent delete ---

pub fn permanently_delete_item(id: &str) -> Result<(), String> {
    let mut items = read_index();
    let pos = items
        .iter()
        .position(|i| i.id == id)
        .ok_or_else(|| format!("Item {} not found in trash", id))?;
    let item = items[pos].clone();

    if matches!(item.item_type, TrashItemType::Skill) {
        let trash_skill_path = skills_dir().join(&item.id);
        if trash_skill_path.exists() {
            remove_path_any(&trash_skill_path)
                .map_err(|e| format!("remove trash path {}: {}", trash_skill_path.display(), e))?;
        }
    }

    items.remove(pos);
    save_index(&items)?;
    Ok(())
}

// --- Empty trash ---

pub fn empty_trash() -> Result<(), String> {
    let trash = trash_dir();
    if trash.exists() {
        remove_path_any(&trash)?;
    }
    Ok(())
}

// --- List ---

pub fn list_trash() -> Vec<TrashItem> {
    read_index()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    // Tests modify HOME env var which is not thread-safe; serialize them
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn setup_test_trash() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        dir
    }

    #[test]
    fn test_empty_trash() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _dir = setup_test_trash();
        // Add a skill item
        let item = TrashItem {
            id: "test-id-1".to_string(),
            item_type: TrashItemType::Skill,
            name: "test-skill".to_string(),
            platform_id: "test-platform".to_string(),
            folder: None,
            original_path: Some("/tmp/test".to_string()),
            original_config: None,
            deleted_at: now_secs(),
        };
        add_item(item).unwrap();

        // Verify item exists
        assert_eq!(list_trash().len(), 1);

        // Empty trash
        empty_trash().unwrap();

        // Verify trash is empty
        assert_eq!(list_trash().len(), 0);
        assert!(!trash_dir().exists());
    }

    #[test]
    fn test_permanently_delete_item() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _dir = setup_test_trash();
        // Add a skill item
        let item = TrashItem {
            id: "test-id-2".to_string(),
            item_type: TrashItemType::Skill,
            name: "test-skill".to_string(),
            platform_id: "test-platform".to_string(),
            folder: None,
            original_path: Some("/tmp/test".to_string()),
            original_config: None,
            deleted_at: now_secs(),
        };
        add_item(item).unwrap();

        // Create a fake skill directory
        let skill_dir = skills_dir().join("test-id-2");
        fs::create_dir_all(&skill_dir).unwrap();
        let mut file = fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        file.write_all(b"test content").unwrap();

        // Verify item and directory exist
        assert_eq!(list_trash().len(), 1);
        assert!(skill_dir.exists());

        // Permanently delete
        permanently_delete_item("test-id-2").unwrap();

        // Verify item and directory are gone
        assert_eq!(list_trash().len(), 0);
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_permanently_delete_mcp_item() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _dir = setup_test_trash();
        // Add an MCP item
        let item = TrashItem {
            id: "test-id-3".to_string(),
            item_type: TrashItemType::Mcp,
            name: "test-mcp".to_string(),
            platform_id: "test-platform".to_string(),
            folder: None,
            original_path: None,
            original_config: Some(serde_json::json!({"key": "value"})),
            deleted_at: now_secs(),
        };
        add_item(item).unwrap();

        // Verify item exists
        assert_eq!(list_trash().len(), 1);

        // Permanently delete
        permanently_delete_item("test-id-3").unwrap();

        // Verify item is gone
        assert_eq!(list_trash().len(), 0);
    }

    #[test]
    fn test_permanently_delete_skill_item_when_trash_path_is_file() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _dir = setup_test_trash();

        let item = TrashItem {
            id: "test-id-4".to_string(),
            item_type: TrashItemType::Skill,
            name: "broken-skill".to_string(),
            platform_id: "test-platform".to_string(),
            folder: None,
            original_path: Some("/tmp/test".to_string()),
            original_config: None,
            deleted_at: now_secs(),
        };
        add_item(item).unwrap();

        // A skill trash path can also be a file (not just a directory).
        fs::create_dir_all(skills_dir()).unwrap();
        let skill_path = skills_dir().join("test-id-4");
        fs::write(&skill_path, "not a directory").unwrap();

        permanently_delete_item("test-id-4").unwrap();
        assert_eq!(list_trash().len(), 0);
        assert!(!skill_path.exists());
    }
}
