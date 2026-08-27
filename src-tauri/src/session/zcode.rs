use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "zcode";

pub fn count_zcode_sessions() -> Result<usize, String> {
    let db_path = zcode_tasks_db_path()?;
    if !db_path.exists() {
        return Ok(0);
    }

    let conn = open_zcode_db_readonly(&db_path)?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks WHERE deleted = 0", [], |row| {
            row.get(0)
        })
        .map_err(|err| err.to_string())?;
    usize::try_from(count).map_err(|err| err.to_string())
}

pub fn list_zcode_sessions_all() -> Result<Vec<SessionSummary>, String> {
    let db_path = zcode_tasks_db_path()?;
    list_zcode_sessions_in_db(&db_path)
}

pub fn get_zcode_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let db_path = zcode_messages_db_path()?;
    read_zcode_messages_from_db(&db_path, session_id, offset, limit)
}

pub fn delete_zcode_session(session_id: &str) -> Result<(), String> {
    let db_path = zcode_tasks_db_path()?;
    delete_zcode_session_in_db(&db_path, session_id)
}

pub fn search_zcode_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let sessions = list_zcode_sessions_all()?;
    let db_path = zcode_messages_db_path()?;
    let mut results = Vec::new();
    for session in sessions {
        let Ok(messages) = read_zcode_messages_from_db(&db_path, &session.id, 0, usize::MAX) else {
            continue;
        };
        for msg in messages {
            if msg.content.to_lowercase().contains(query_lower) {
                results.push(crate::session::SessionSearchResult {
                    session_id: session.id.clone(),
                    session_title: session.title.clone(),
                    project_path: session.project_path.clone(),
                    platform_id: PLATFORM_ID.to_string(),
                    message: msg,
                });
            }
        }
    }
    Ok(results)
}

/// ZCode keeps its session list in `~/.zcode/v2/tasks-index.sqlite` (the same
/// `tasks` table the Electron app's sidebar reads). Subagent transcripts
/// (`sess_subagent_*`) live only in the messages DB and are not listed here,
/// matching what the user sees in the app.
fn list_zcode_sessions_in_db(db_path: &Path) -> Result<Vec<SessionSummary>, String> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = open_zcode_db_readonly(db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT task_id, title, workspace_path, model, created_at, updated_at \
             FROM tasks WHERE deleted = 0 ORDER BY updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let project_path: String = row.get(2)?;
            let model: Option<String> = row.get(3)?;
            let created_at: i64 = row.get(4)?;
            let updated_at: i64 = row.get(5)?;

            let title = if title.trim().is_empty() {
                id.clone()
            } else {
                title
            };

            Ok(SessionSummary {
                id,
                title,
                project_path,
                model: model.as_deref().map(zcode_display_model),
                started_at: created_at,
                updated_at,
                message_count: None,
                tokens_used: None,
                platform_id: PLATFORM_ID.to_string(),
                source: None,
            })
        })
        .map_err(|err| err.to_string())?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(|err| err.to_string())?);
    }
    Ok(sessions)
}

/// Tasks record the model as `<provider>/<model>` (e.g.
/// `builtin:bigmodel-coding-plan/GLM-5.2`); only the model name is useful in
/// the UI.
fn zcode_display_model(raw: &str) -> String {
    raw.rsplit('/').next().unwrap_or(raw).to_string()
}

/// Message bodies live in a separate database (`~/.zcode/cli/db/db.sqlite`):
/// `message.data` is a JSON blob whose `role` is user|assistant, and the
/// visible text is the concatenation of the message's `part` rows whose
/// `data.type` is "text" (reasoning/tool/step parts are skipped).
fn read_zcode_messages_from_db(
    db_path: &Path,
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let conn = open_zcode_db_readonly(db_path)?;
    let mut msg_stmt = conn
        .prepare(
            "SELECT id, time_created, data FROM message \
             WHERE session_id = ?1 ORDER BY time_created, id",
        )
        .map_err(|err| err.to_string())?;
    let mut part_stmt = conn
        .prepare("SELECT data FROM part WHERE message_id = ?1 ORDER BY time_created, id")
        .map_err(|err| err.to_string())?;

    let rows = msg_stmt
        .query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|err| err.to_string())?;

    let mut messages = Vec::new();
    let mut matched = 0usize;
    let page_limit = limit.max(1);

    for row in rows {
        let (message_id, time_created, data) = row.map_err(|err| err.to_string())?;
        let data: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let role = match data.get("role").and_then(|v| v.as_str()) {
            Some("user") => "user",
            Some("assistant") => "assistant",
            _ => continue,
        };

        let content = read_zcode_message_text(&mut part_stmt, &message_id)?;
        if content.is_empty() {
            continue;
        }

        if matched >= offset {
            messages.push(SessionMessage::new(role, content, time_created));
            if messages.len() >= page_limit {
                break;
            }
        }
        matched += 1;
    }

    Ok(messages)
}

fn read_zcode_message_text(
    part_stmt: &mut rusqlite::Statement,
    message_id: &str,
) -> Result<String, String> {
    let parts = part_stmt
        .query_map([message_id], |row| row.get::<_, String>(0))
        .map_err(|err| err.to_string())?;

    let mut texts = Vec::new();
    for part in parts {
        let data: Value = match part.ok().and_then(|raw| serde_json::from_str(&raw).ok()) {
            Some(value) => value,
            None => continue,
        };
        if data.get("type").and_then(|v| v.as_str()) != Some("text") {
            continue;
        }
        let Some(text) = data.get("text").and_then(|v| v.as_str()) else {
            continue;
        };
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            texts.push(trimmed.to_string());
        }
    }
    Ok(texts.join("\n"))
}

/// Deleting a ZCode session only flips the `deleted` flag in the task index —
/// the app treats it as gone and the messages DB is left untouched, matching
/// the Codex adapter's archive-only behavior.
fn delete_zcode_session_in_db(db_path: &Path, session_id: &str) -> Result<(), String> {
    if !db_path.exists() {
        return Err(format!(
            "ZCode session database not found: {}",
            db_path.display()
        ));
    }

    let conn = open_zcode_db_readwrite(db_path)?;
    let changed = conn
        .execute(
            "UPDATE tasks SET deleted = 1 WHERE task_id = ?1 AND deleted = 0",
            [session_id],
        )
        .map_err(|err| err.to_string())?;
    if changed > 0 {
        return Ok(());
    }

    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE task_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    if exists > 0 {
        return Err(format!("ZCode session already deleted: {}", session_id));
    }
    Err(format!("ZCode session not found: {}", session_id))
}

fn zcode_tasks_db_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(join_relative(home, ".zcode/v2/tasks-index.sqlite"))
}

fn zcode_messages_db_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(join_relative(home, ".zcode/cli/db/db.sqlite"))
}

fn open_zcode_db_readonly(path: &Path) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    open_zcode_db_with_flags(path, flags)
}

fn open_zcode_db_readwrite(path: &Path) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    open_zcode_db_with_flags(path, flags)
}

fn open_zcode_db_with_flags(path: &Path, flags: OpenFlags) -> Result<Connection, String> {
    for attempt in 0..3 {
        match Connection::open_with_flags(path, flags) {
            Ok(connection) => {
                let _ = connection.busy_timeout(Duration::from_millis(1500));
                return Ok(connection);
            }
            Err(error) => {
                if attempt < 2 {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                return Err(error.to_string());
            }
        }
    }
    Err(format!("Unable to open ZCode database: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_tasks_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().expect("temp dir should create");
        let db_path = dir.path().join("tasks-index.sqlite");
        let conn = Connection::open(&db_path).expect("sqlite db should create");
        conn.execute_batch(
            "CREATE TABLE tasks (
                workspace_key TEXT NOT NULL,
                workspace_path TEXT NOT NULL,
                task_id TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                task_status TEXT,
                model TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                archived INTEGER NOT NULL DEFAULT 0,
                deleted INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (workspace_key, task_id)
            );",
        )
        .expect("tasks table should create");
        (dir, db_path)
    }

    fn insert_task(db_path: &Path, task_id: &str, title: &str, deleted: i64, updated_at: i64) {
        let conn = Connection::open(db_path).expect("db should open");
        conn.execute(
            "INSERT INTO tasks (workspace_key, workspace_path, task_id, title, model, created_at, updated_at, deleted)
             VALUES ('wk', '/tmp/work', ?1, ?2, 'builtin:bigmodel-coding-plan/GLM-5.2', 1000, ?3, ?4)",
            (task_id, title, updated_at, deleted),
        )
        .expect("task should insert");
    }

    fn create_test_messages_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().expect("temp dir should create");
        let db_path = dir.path().join("db.sqlite");
        let conn = Connection::open(&db_path).expect("sqlite db should create");
        conn.execute_batch(
            "CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE part (
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data TEXT NOT NULL
            );",
        )
        .expect("message tables should create");
        (dir, db_path)
    }

    fn insert_message(
        db_path: &Path,
        message_id: &str,
        session_id: &str,
        time_created: i64,
        data: &str,
    ) {
        let conn = Connection::open(db_path).expect("db should open");
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4)",
            (message_id, session_id, time_created, data),
        )
        .expect("message should insert");
    }

    fn insert_part(
        db_path: &Path,
        part_id: &str,
        message_id: &str,
        session_id: &str,
        time_created: i64,
        data: &str,
    ) {
        let conn = Connection::open(db_path).expect("db should open");
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            (part_id, message_id, session_id, time_created, data),
        )
        .expect("part should insert");
    }

    #[test]
    fn list_zcode_sessions_filters_deleted_and_sorts_by_updated_desc() {
        let (_dir, db_path) = create_test_tasks_db();
        insert_task(&db_path, "sess_old", "old", 0, 100);
        insert_task(&db_path, "sess_new", "new", 0, 200);
        insert_task(&db_path, "sess_gone", "gone", 1, 300);

        let sessions = list_zcode_sessions_in_db(&db_path).expect("list should succeed");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "sess_new");
        assert_eq!(sessions[1].id, "sess_old");
        assert_eq!(sessions[0].platform_id, "zcode");
        assert_eq!(sessions[0].project_path, "/tmp/work");
        // Provider prefix is stripped from the raw model string.
        assert_eq!(sessions[0].model.as_deref(), Some("GLM-5.2"));
    }

    #[test]
    fn list_zcode_sessions_falls_back_to_id_as_title() {
        let (_dir, db_path) = create_test_tasks_db();
        insert_task(&db_path, "sess_1", "  ", 0, 100);

        let sessions = list_zcode_sessions_in_db(&db_path).expect("list should succeed");
        assert_eq!(sessions[0].title, "sess_1");
    }

    #[test]
    fn list_zcode_sessions_missing_db_is_empty() {
        let mut missing = std::env::temp_dir();
        missing.push(format!(
            "agent-hub-zcode-missing-{}.sqlite",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        let sessions = list_zcode_sessions_in_db(&missing).expect("missing db is empty list");
        assert!(sessions.is_empty());
    }

    #[test]
    fn read_zcode_messages_joins_text_parts_only() {
        let (_dir, db_path) = create_test_messages_db();
        insert_message(
            &db_path,
            "msg_u1",
            "sess_1",
            10,
            r#"{"role":"user","time":{"created":10}}"#,
        );
        insert_part(
            &db_path,
            "part_u1",
            "msg_u1",
            "sess_1",
            10,
            r#"{"type":"text","text":"你好"}"#,
        );
        insert_message(
            &db_path,
            "msg_a1",
            "sess_1",
            20,
            r#"{"role":"assistant","time":{"created":20}}"#,
        );
        insert_part(
            &db_path,
            "part_r1",
            "msg_a1",
            "sess_1",
            20,
            r#"{"type":"reasoning","text":"思考中"}"#,
        );
        insert_part(
            &db_path,
            "part_t1",
            "msg_a1",
            "sess_1",
            21,
            r#"{"type":"text","text":"第一段"}"#,
        );
        insert_part(
            &db_path,
            "part_t2",
            "msg_a1",
            "sess_1",
            22,
            r#"{"type":"text","text":"第二段"}"#,
        );
        insert_part(
            &db_path,
            "part_tool",
            "msg_a1",
            "sess_1",
            23,
            r#"{"type":"tool","tool":"Read"}"#,
        );

        let messages =
            read_zcode_messages_from_db(&db_path, "sess_1", 0, 50).expect("messages should load");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "你好");
        assert_eq!(messages[0].timestamp, 10);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "第一段\n第二段");
    }

    #[test]
    fn read_zcode_messages_skips_messages_without_text() {
        let (_dir, db_path) = create_test_messages_db();
        // An assistant message with only reasoning/tool parts yields nothing.
        insert_message(&db_path, "msg_a1", "sess_1", 10, r#"{"role":"assistant"}"#);
        insert_part(
            &db_path,
            "part_r1",
            "msg_a1",
            "sess_1",
            10,
            r#"{"type":"reasoning","text":"..."}"#,
        );
        insert_message(&db_path, "msg_u1", "sess_1", 20, r#"{"role":"user"}"#);
        insert_part(
            &db_path,
            "part_u1",
            "msg_u1",
            "sess_1",
            20,
            r#"{"type":"text","text":"问题"}"#,
        );

        let messages =
            read_zcode_messages_from_db(&db_path, "sess_1", 0, 50).expect("messages should load");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "问题");
    }

    #[test]
    fn read_zcode_messages_respects_offset_limit() {
        let (_dir, db_path) = create_test_messages_db();
        for idx in 0..3 {
            let message_id = format!("msg_{idx}");
            let part_id = format!("part_{idx}");
            insert_message(&db_path, &message_id, "sess_1", idx, r#"{"role":"user"}"#);
            insert_part(
                &db_path,
                &part_id,
                &message_id,
                "sess_1",
                idx,
                &format!(r#"{{"type":"text","text":"m{idx}"}}"#),
            );
        }

        let page = read_zcode_messages_from_db(&db_path, "sess_1", 1, 1).expect("page should load");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].content, "m1");
    }

    #[test]
    fn delete_zcode_session_sets_deleted_flag() {
        let (_dir, db_path) = create_test_tasks_db();
        insert_task(&db_path, "sess_1", "t", 0, 100);

        delete_zcode_session_in_db(&db_path, "sess_1").expect("delete should succeed");

        let conn = Connection::open(&db_path).expect("db should reopen");
        let deleted: i64 = conn
            .query_row(
                "SELECT deleted FROM tasks WHERE task_id = ?1",
                ["sess_1"],
                |row| row.get(0),
            )
            .expect("deleted should load");
        assert_eq!(deleted, 1);
    }

    #[test]
    fn delete_zcode_session_reports_missing_session() {
        let (_dir, db_path) = create_test_tasks_db();
        let err = delete_zcode_session_in_db(&db_path, "missing")
            .expect_err("missing session should fail");
        assert!(err.contains("not found"));
    }

    #[test]
    fn delete_zcode_session_reports_already_deleted() {
        let (_dir, db_path) = create_test_tasks_db();
        insert_task(&db_path, "sess_1", "t", 1, 100);
        let err = delete_zcode_session_in_db(&db_path, "sess_1")
            .expect_err("already-deleted session should fail");
        assert!(err.contains("already deleted"));
    }

    #[test]
    fn zcode_display_model_strips_provider_prefix() {
        assert_eq!(
            zcode_display_model("builtin:bigmodel-coding-plan/GLM-5.2"),
            "GLM-5.2"
        );
        assert_eq!(zcode_display_model("GLM-5.2"), "GLM-5.2");
    }
}
