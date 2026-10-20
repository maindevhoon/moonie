use crate::AppState;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub due_at: Option<String>,
    pub scheduled_at: Option<String>,
    pub estimate_min: Option<i64>,
    pub dependency_ids: Vec<String>,
    pub recurrence: String,
    pub source_url: Option<String>,
    pub status: String,
    pub created_at: String,
    pub snoozed_until: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftTask {
    pub title: String,
    #[serde(default)]
    pub notes: String,
    pub due_at: Option<String>,
    pub scheduled_at: Option<String>,
    pub estimate_min: Option<i64>,
    #[serde(default)]
    pub dependency_ids: Vec<String>,
    #[serde(default = "default_recurrence")]
    pub recurrence: String,
    pub source_url: Option<String>,
}
fn default_recurrence() -> String {
    "none".into()
}

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;
      CREATE TABLE IF NOT EXISTS tasks (
        id TEXT PRIMARY KEY, title TEXT NOT NULL, notes TEXT NOT NULL DEFAULT '',
        due_at TEXT, scheduled_at TEXT, estimate_min INTEGER,
        dependency_ids TEXT NOT NULL DEFAULT '[]', recurrence TEXT NOT NULL DEFAULT 'none',
        source_url TEXT, status TEXT NOT NULL DEFAULT 'open',
        created_at TEXT NOT NULL, snoozed_until TEXT, reminded_at TEXT,
        recurrence_generated INTEGER NOT NULL DEFAULT 0
      );
      CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS approval_batches (id TEXT PRIMARY KEY, created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS focus_state (id INTEGER PRIMARY KEY CHECK(id=1), started_at INTEGER, accumulated_seconds INTEGER NOT NULL DEFAULT 0, active INTEGER NOT NULL DEFAULT 0);
      INSERT OR IGNORE INTO focus_state(id) VALUES (1);
      CREATE INDEX IF NOT EXISTS tasks_status_due ON tasks(status, due_at);")?;
    let mut stmt = conn.prepare("PRAGMA table_info(tasks)")?;
    let columns: Vec<String> = stmt
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<_>>()?;
    if !columns.iter().any(|c| c == "recurrence_generated") {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN recurrence_generated INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    conn.pragma_update(None, "user_version", 1)?;
    Ok(())
}

pub fn validate(d: &DraftTask) -> Result<(), String> {
    if d.title.trim().is_empty() || d.title.chars().count() > 160 {
        return Err("Task title must be 1–160 characters".into());
    }
    if d.notes.chars().count() > 4000 {
        return Err("Task notes are too long".into());
    }
    if !["none", "daily", "weekly"].contains(&d.recurrence.as_str()) {
        return Err("Invalid recurrence".into());
    }
    if let Some(n) = d.estimate_min
        && !(1..=1440).contains(&n)
    {
        return Err("Estimate must be 1–1440 minutes".into());
    }
    for value in [&d.due_at, &d.scheduled_at].into_iter().flatten() {
        DateTime::parse_from_rfc3339(value)
            .map_err(|_| "Dates must use an ISO timestamp with time zone")?;
    }
    if let Some(url) = &d.source_url {
        let parsed = url::Url::parse(url).map_err(|_| "Invalid source URL")?;
        if !["http", "https"].contains(&parsed.scheme()) {
            return Err("Source URL must be HTTP(S)".into());
        }
    }
    Ok(())
}

fn validate_dependencies(
    conn: &Connection,
    draft: &DraftTask,
    edited_id: Option<&str>,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for id in &draft.dependency_ids {
        if !seen.insert(id) || edited_id == Some(id.as_str()) {
            return Err("A task cannot depend on itself or repeat a dependency".into());
        }
        let found: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?)", [id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if !found {
            return Err("A dependency task no longer exists".into());
        }
        if let Some(edited_id) = edited_id
            && reaches(conn, id, edited_id, &mut std::collections::HashSet::new())?
        {
            return Err("Task dependencies cannot form a cycle".into());
        }
    }
    Ok(())
}

fn reaches(
    conn: &Connection,
    from: &str,
    target: &str,
    visited: &mut std::collections::HashSet<String>,
) -> Result<bool, String> {
    if from == target {
        return Ok(true);
    }
    if !visited.insert(from.to_string()) {
        return Ok(false);
    }
    let json: String = conn
        .query_row("SELECT dependency_ids FROM tasks WHERE id=?", [from], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?;
    let dependencies: Vec<String> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    for id in dependencies {
        if reaches(conn, &id, target, visited)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn next_occurrence(value: Option<String>, recurrence: &str) -> Option<String> {
    let days = if recurrence == "daily" { 1 } else { 7 };
    value
        .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
        .map(|mut v| {
            loop {
                v += Duration::days(days);
                if v.with_timezone(&Utc) > Utc::now() {
                    break v.to_rfc3339();
                }
            }
        })
}

fn task_from_row(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let dependencies: String = row.get(6)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        due_at: row.get(3)?,
        scheduled_at: row.get(4)?,
        estimate_min: row.get(5)?,
        dependency_ids: serde_json::from_str(&dependencies).unwrap_or_default(),
        recurrence: row.get(7)?,
        source_url: row.get(8)?,
        status: row.get(9)?,
        created_at: row.get(10)?,
        snoozed_until: row.get(11)?,
    })
}

fn all(conn: &Connection) -> rusqlite::Result<Vec<Task>> {
    let mut stmt = conn.prepare("SELECT id,title,notes,due_at,scheduled_at,estimate_min,dependency_ids,recurrence,source_url,status,created_at,snoozed_until FROM tasks ORDER BY created_at DESC")?;
    stmt.query_map([], task_from_row)?.collect()
}

#[tauri::command]
pub fn list_tasks(state: State<AppState>) -> Result<Vec<Task>, String> {
    let conn = state.db.lock().map_err(|_| "Database unavailable")?;
    all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn approve_drafts(
    app: tauri::AppHandle,
    state: State<AppState>,
    drafts: Vec<DraftTask>,
    batch_id: Option<String>,
) -> Result<Vec<Task>, String> {
    if drafts.is_empty() || drafts.len() > 30 {
        return Err("Approve between 1 and 30 tasks".into());
    }
    for d in &drafts {
        validate(d)?;
    }
    let mut conn = state.db.lock().map_err(|_| "Database unavailable")?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if let Some(batch_id) = batch_id {
        if batch_id.len() > 100 || batch_id.is_empty() {
            return Err("Invalid approval batch".into());
        }
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO approval_batches(id,created_at) VALUES (?,?)",
                params![batch_id, Utc::now().to_rfc3339()],
            )
            .map_err(|e| e.to_string())?;
        if inserted == 0 {
            drop(tx);
            return all(&conn).map_err(|e| e.to_string());
        }
    }
    for d in drafts {
        validate_dependencies(&tx, &d, None)?;
        tx.execute("INSERT INTO tasks (id,title,notes,due_at,scheduled_at,estimate_min,dependency_ids,recurrence,source_url,created_at) VALUES (?,?,?,?,?,?,?,?,?,?)",
            params![Uuid::new_v4().to_string(), d.title.trim(), d.notes, d.due_at, d.scheduled_at, d.estimate_min, serde_json::to_string(&d.dependency_ids).unwrap(), d.recurrence, d.source_url, Utc::now().to_rfc3339()])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    let tasks = all(&conn).map_err(|e| e.to_string())?;
    let _ = app.emit("tasks-changed", ());
    let _ = app.emit("mascot-state", "encouraging");
    Ok(tasks)
}

#[tauri::command]
pub fn save_manual_task(
    app: tauri::AppHandle,
    state: State<AppState>,
    draft: DraftTask,
) -> Result<Vec<Task>, String> {
    approve_drafts(app, state, vec![draft], None)
}

#[tauri::command]
pub fn update_task(
    app: tauri::AppHandle,
    state: State<AppState>,
    id: String,
    draft: DraftTask,
    status: String,
    snoozed_until: Option<String>,
) -> Result<Vec<Task>, String> {
    validate(&draft)?;
    if !["open", "done"].contains(&status.as_str()) {
        return Err("Invalid status".into());
    }
    if let Some(ref until) = snoozed_until {
        DateTime::parse_from_rfc3339(until).map_err(|_| "Invalid snooze date")?;
    }
    let mut conn = state.db.lock().map_err(|_| "Database unavailable")?;
    validate_dependencies(&conn, &draft, Some(&id))?;
    let old: (String, i64) = conn
        .query_row(
            "SELECT status,recurrence_generated FROM tasks WHERE id=?",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "Task not found")?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let affected = tx.execute("UPDATE tasks SET title=?,notes=?,due_at=?,scheduled_at=?,estimate_min=?,dependency_ids=?,recurrence=?,source_url=?,status=?,snoozed_until=?,reminded_at=NULL WHERE id=?",
        params![draft.title.trim(),draft.notes,draft.due_at,draft.scheduled_at,draft.estimate_min,serde_json::to_string(&draft.dependency_ids).unwrap(),draft.recurrence,draft.source_url,status,snoozed_until,id]).map_err(|e| e.to_string())?;
    if affected == 0 {
        return Err("Task not found".into());
    }
    if status == "done" && old.0 == "open" && old.1 == 0 && draft.recurrence != "none" {
        let next_schedule = if draft.due_at.is_none() && draft.scheduled_at.is_none() {
            Some(
                (Utc::now() + Duration::days(if draft.recurrence == "daily" { 1 } else { 7 }))
                    .to_rfc3339(),
            )
        } else {
            next_occurrence(draft.scheduled_at, &draft.recurrence)
        };
        tx.execute("INSERT INTO tasks (id,title,notes,due_at,scheduled_at,estimate_min,dependency_ids,recurrence,source_url,created_at) VALUES (?,?,?,?,?,?,?,?,?,?)",
            params![Uuid::new_v4().to_string(),draft.title.trim(),draft.notes,next_occurrence(draft.due_at, &draft.recurrence),next_schedule,draft.estimate_min,serde_json::to_string(&draft.dependency_ids).unwrap(),draft.recurrence,draft.source_url,Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
        tx.execute("UPDATE tasks SET recurrence_generated=1 WHERE id=?", [&id])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    let tasks = all(&conn).map_err(|e| e.to_string())?;
    let _ = app.emit("tasks-changed", ());
    if status == "done" {
        let _ = app.emit("mascot-state", "celebration");
    }
    Ok(tasks)
}

#[tauri::command]
pub fn delete_task(
    app: tauri::AppHandle,
    state: State<AppState>,
    id: String,
) -> Result<Vec<Task>, String> {
    let mut conn = state.db.lock().map_err(|_| "Database unavailable")?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM tasks WHERE id=?", [&id])
        .map_err(|e| e.to_string())?;
    {
        let mut stmt = tx
            .prepare("SELECT id,dependency_ids FROM tasks")
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<_>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        for (task_id, json) in rows {
            let mut deps: Vec<String> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let previous = deps.len();
            deps.retain(|dep| dep != &id);
            if deps.len() != previous {
                tx.execute(
                    "UPDATE tasks SET dependency_ids=? WHERE id=?",
                    params![serde_json::to_string(&deps).unwrap(), task_id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    let tasks = all(&conn).map_err(|e| e.to_string())?;
    let _ = app.emit("tasks-changed", ());
    Ok(tasks)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub task: Task,
    pub reason: String,
}

pub fn select_next(tasks: &[Task], available_min: Option<i64>) -> Option<Suggestion> {
    let now = Utc::now();
    let mut ranked: Vec<(&Task, i32, i64, String)> = tasks
        .iter()
        .filter_map(|t| {
            if t.status != "open" {
                return None;
            }
            if t.dependency_ids.iter().any(|id| {
                tasks
                    .iter()
                    .find(|other| &other.id == id)
                    .is_none_or(|other| other.status != "done")
            }) {
                return None;
            }
            if t.snoozed_until
                .as_deref()
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                .is_some_and(|d| d > now)
            {
                return None;
            }
            if available_min.is_some_and(|n| t.estimate_min.is_some_and(|estimate| estimate > n)) {
                return None;
            }
            let scheduled = t
                .scheduled_at
                .as_deref()
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok());
            let due = t
                .due_at
                .as_deref()
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok());
            if scheduled.is_some_and(|d| d > now) {
                return None;
            }
            let (rank, reason) = if scheduled.is_some_and(|d| d <= now) {
                (0, "Scheduled for now")
            } else if due.is_some_and(|d| d < now) {
                (1, "Past its deadline")
            } else if due.is_some() {
                (2, "Upcoming deadline")
            } else {
                (3, "Available flexible task")
            };
            Some((
                t,
                rank,
                due.map(|d| d.timestamp()).unwrap_or(i64::MAX),
                reason.to_string(),
            ))
        })
        .collect();
    ranked.sort_by(|a, b| (a.1, &a.2, &a.0.created_at).cmp(&(b.1, &b.2, &b.0.created_at)));
    ranked.first().map(|(t, _, _, reason)| Suggestion {
        task: (*t).clone(),
        reason: reason.clone(),
    })
}

#[tauri::command]
pub fn next_task(
    state: State<AppState>,
    available_min: Option<i64>,
) -> Result<Option<Suggestion>, String> {
    let conn = state.db.lock().map_err(|_| "Database unavailable")?;
    let tasks = all(&conn).map_err(|e| e.to_string())?;
    Ok(select_next(&tasks, available_min))
}

#[tauri::command]
pub fn get_settings(
    state: State<AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let conn = state.db.lock().map_err(|_| "Database unavailable")?;
    let mut stmt = conn
        .prepare("SELECT key,value FROM settings")
        .map_err(|e| e.to_string())?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(
    app: tauri::AppHandle,
    state: State<AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    if ![
        "model",
        "quiet",
        "reducedMotion",
        "animationPaused",
        "mascotSize",
        "mascotX",
        "mascotY",
    ]
    .contains(&key.as_str())
    {
        return Err("Unknown setting".into());
    }
    if value.len() > 300 {
        return Err("Setting too long".into());
    }
    state.db.lock().map_err(|_| "Database unavailable")?.execute("INSERT INTO settings(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map_err(|e| e.to_string())?;
    let _ = app.emit("settings-changed", ());
    Ok(())
}

pub fn due_reminders(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT id,title,due_at,scheduled_at,snoozed_until FROM tasks WHERE status='open' AND reminded_at IS NULL")?;
    type ReminderCandidate = (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let candidates: Vec<ReminderCandidate> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let current = Utc::now();
    let rows: Vec<(String, String)> = candidates
        .into_iter()
        .filter(|(_, _, due, scheduled, snoozed)| {
            let parse = |s: &String| {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            };
            let ready = due.as_ref().and_then(parse).is_some_and(|d| d <= current)
                || scheduled
                    .as_ref()
                    .and_then(parse)
                    .is_some_and(|d| d <= current);
            ready
                && !snoozed
                    .as_ref()
                    .and_then(parse)
                    .is_some_and(|d| d > current)
        })
        .map(|(id, title, _, _, _)| (id, title))
        .collect();
    Ok(rows)
}

pub fn mark_reminders_delivered(
    conn: &Connection,
    rows: &[(String, String)],
) -> rusqlite::Result<()> {
    let now = Utc::now().to_rfc3339();
    for (id, _) in rows {
        conn.execute(
            "UPDATE tasks SET reminded_at=? WHERE id=?",
            params![now, id],
        )?;
    }
    Ok(())
}

pub fn setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
        r.get(0)
    })
    .optional()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusStatus {
    pub seconds: i64,
    pub active: bool,
}

fn focus_status(conn: &Connection) -> rusqlite::Result<FocusStatus> {
    let (started, accumulated, active): (Option<i64>, i64, bool) = conn.query_row(
        "SELECT started_at,accumulated_seconds,active FROM focus_state WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let seconds = accumulated
        + if active {
            Utc::now()
                .timestamp()
                .saturating_sub(started.unwrap_or_else(|| Utc::now().timestamp()))
        } else {
            0
        };
    Ok(FocusStatus { seconds, active })
}

#[tauri::command]
pub fn get_focus(state: State<AppState>) -> Result<FocusStatus, String> {
    let conn = state.db.lock().map_err(|_| "Database unavailable")?;
    focus_status(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn focus_action(
    app: tauri::AppHandle,
    state: State<AppState>,
    action: String,
) -> Result<FocusStatus, String> {
    let conn = state.db.lock().map_err(|_| "Database unavailable")?;
    let old = focus_status(&conn).map_err(|e| e.to_string())?;
    match action.as_str() {
        "start" if !old.active => conn
            .execute(
                "UPDATE focus_state SET started_at=?,active=1 WHERE id=1",
                [Utc::now().timestamp()],
            )
            .map_err(|e| e.to_string())?,
        "pause" if old.active => conn
            .execute(
                "UPDATE focus_state SET started_at=NULL,accumulated_seconds=?,active=0 WHERE id=1",
                [old.seconds],
            )
            .map_err(|e| e.to_string())?,
        "reset" => conn
            .execute(
                "UPDATE focus_state SET started_at=NULL,accumulated_seconds=0,active=0 WHERE id=1",
                [],
            )
            .map_err(|e| e.to_string())?,
        "break" => conn
            .execute(
                "UPDATE focus_state SET started_at=NULL,accumulated_seconds=?,active=0 WHERE id=1",
                [old.seconds],
            )
            .map_err(|e| e.to_string())?,
        "start" | "pause" => 0,
        _ => return Err("Unknown focus action".into()),
    };
    let _ = app.emit(
        "mascot-state",
        match action.as_str() {
            "start" => "focused",
            "break" => "resting",
            _ => "idle",
        },
    );
    focus_status(&conn).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, status: &str, due_at: Option<String>, dependencies: Vec<&str>) -> Task {
        Task {
            id: id.into(),
            title: id.into(),
            notes: String::new(),
            due_at,
            scheduled_at: None,
            estimate_min: None,
            dependency_ids: dependencies.into_iter().map(str::to_string).collect(),
            recurrence: "none".into(),
            source_url: None,
            status: status.into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            snoozed_until: None,
        }
    }

    #[test]
    fn next_task_skips_blocked_and_completed() {
        let soon = (Utc::now() + Duration::hours(1)).to_rfc3339();
        let tasks = vec![
            task("prerequisite", "open", None, vec![]),
            task("blocked", "open", Some(soon.clone()), vec!["prerequisite"]),
            task("done", "done", Some(soon), vec![]),
        ];
        assert_eq!(select_next(&tasks, None).unwrap().task.id, "prerequisite");
    }

    #[test]
    fn cycle_is_rejected() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute("INSERT INTO tasks(id,title,created_at,dependency_ids) VALUES ('a','A','now','[\"b\"]'),('b','B','now','[]')", []).unwrap();
        let draft = DraftTask {
            title: "B".into(),
            notes: String::new(),
            due_at: None,
            scheduled_at: None,
            estimate_min: None,
            dependency_ids: vec!["a".into()],
            recurrence: "none".into(),
            source_url: None,
        };
        assert!(
            validate_dependencies(&conn, &draft, Some("b"))
                .unwrap_err()
                .contains("cycle")
        );
    }

    #[test]
    fn recurrence_moves_past_dates_forward() {
        let next = next_occurrence(Some("2020-01-01T09:00:00+05:30".into()), "daily").unwrap();
        assert!(
            DateTime::parse_from_rfc3339(&next)
                .unwrap()
                .with_timezone(&Utc)
                > Utc::now()
        );
    }

    #[test]
    fn suggestions_respect_time_snoozes_and_future_schedule() {
        let mut scheduled = task("later", "open", None, vec![]);
        scheduled.scheduled_at = Some((Utc::now() + Duration::hours(1)).to_rfc3339());
        let mut snoozed = task("snoozed", "open", None, vec![]);
        snoozed.snoozed_until = scheduled.scheduled_at.clone();
        let mut long = task("long", "open", None, vec![]);
        long.estimate_min = Some(60);
        let short = task("short", "open", None, vec![]);
        assert_eq!(
            select_next(&[scheduled, snoozed, long, short], Some(15))
                .unwrap()
                .task
                .id,
            "short"
        );
    }

    #[test]
    fn reminders_compare_timezones_and_mark_only_after_delivery() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let past = (Utc::now() - Duration::minutes(2))
            .with_timezone(&chrono::FixedOffset::east_opt(19800).unwrap())
            .to_rfc3339();
        conn.execute(
            "INSERT INTO tasks(id,title,due_at,created_at) VALUES ('due','Due',?,'now')",
            [past],
        )
        .unwrap();
        let first = due_reminders(&conn).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(due_reminders(&conn).unwrap().len(), 1);
        mark_reminders_delivered(&conn, &first).unwrap();
        assert!(due_reminders(&conn).unwrap().is_empty());
    }

    #[test]
    fn migration_adds_recurrence_tracking_to_existing_database() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY, title TEXT NOT NULL, notes TEXT NOT NULL DEFAULT '', due_at TEXT, scheduled_at TEXT, estimate_min INTEGER, dependency_ids TEXT NOT NULL DEFAULT '[]', recurrence TEXT NOT NULL DEFAULT 'none', source_url TEXT, status TEXT NOT NULL DEFAULT 'open', created_at TEXT NOT NULL, snoozed_until TEXT, reminded_at TEXT);").unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO tasks(id,title,created_at) VALUES ('1','Sample','now')",
            [],
        )
        .unwrap();
        let value: i64 = conn
            .query_row("SELECT recurrence_generated FROM tasks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, 0);
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
