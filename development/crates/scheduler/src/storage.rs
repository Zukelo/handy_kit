use crate::model::{Run, Task};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;
pub type Result<T> = std::result::Result<T, String>;
pub fn migrate(conn: &mut Connection, path: &Path) -> Result<()> {
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS schema_migrations(version TEXT PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").map_err(|e|e.to_string())?;
    let migrations = [
        (
            "0001_initial.sql",
            include_str!("../../../../database/migrations/0001_initial.sql"),
        ),
        (
            "0002_timer_runtime.sql",
            include_str!("../../../../database/migrations/0002_timer_runtime.sql"),
        ),
    ];
    {
        let mut stmt = conn
            .prepare("SELECT version FROM schema_migrations")
            .map_err(|e| e.to_string())?;
        let versions = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for version in versions {
            let version = version.map_err(|e| e.to_string())?;
            if !migrations.iter().any(|(name, _)| *name == version) {
                return Err(format!(
                    "数据库包含当前版本不支持的迁移：{version}，请使用更新版本小工坊"
                ));
            }
        }
    }
    let mut backed_up = false;
    for (version, sql) in migrations {
        let checksum = format!("{:x}", Sha256::digest(sql.as_bytes()));
        let old: Option<String> = conn
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version=?",
                [version],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(old) = old {
            if old != checksum {
                return Err(format!("迁移校验和不符：{version}，未启动调度器"));
            }
            continue;
        }
        if !backed_up
            && path.exists()
            && conn
                .query_row("SELECT count(*) FROM schema_migrations", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap_or(0)
                > 0
        {
            let backup =
                path.with_extension(format!("{}.backup", chrono::Utc::now().timestamp_millis()));
            conn.backup(
                rusqlite::MAIN_DB,
                &backup,
                None::<fn(rusqlite::backup::Progress)>,
            )
            .map_err(|e| format!("升级前备份失败：{e}"))?;
            backed_up = true;
        }
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql).map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO schema_migrations(version,checksum) VALUES (?,?)",
            params![version, checksum],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| e.to_string())?;
    conn.execute("UPDATE timer_runs SET status='interrupted',finished_at=?,message='上次进程退出，外部动作结果未知；不会自动重跑' WHERE status='running'",[chrono::Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn
        .prepare("SELECT config_json FROM timer_tasks WHERE deleted=0 ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| {
        serde_json::from_str(&r.map_err(|e| e.to_string())?)
            .map_err(|e| format!("任务配置损坏：{e}"))
    })
    .collect()
}
pub fn task(conn: &Connection, id: &str) -> Result<Task> {
    let json: String = conn
        .query_row(
            "SELECT config_json FROM timer_tasks WHERE id=? AND deleted=0",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| "任务不存在或已删除".to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}
pub fn row_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
    let json: String = r.get(13)?;
    let snapshot = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(13, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(Run {
        id: r.get(0)?,
        task_id: r.get(1)?,
        revision: r.get(2)?,
        source: r.get(3)?,
        scheduled_at: r.get(4)?,
        started_at: r.get(5)?,
        finished_at: r.get(6)?,
        status: r.get(7)?,
        exit_code: r.get(8)?,
        message: r.get(9)?,
        stdout: r.get(10)?,
        stderr: r.get(11)?,
        truncated: r.get(12)?,
        snapshot,
    })
}
// Named projection prevents SELECT * drift.
const COLUMNS:&str="id,task_id,revision,source,scheduled_at,started_at,finished_at,status,exit_code,message,stdout,stderr,truncated,snapshot_json";
pub fn latest(conn: &Connection, id: &str) -> Result<Option<Run>> {
    let sql = format!(
        "SELECT {} FROM timer_runs WHERE task_id=? ORDER BY started_at DESC,rowid DESC LIMIT 1",
        COLUMNS.replace("stdout,stderr", "'' AS stdout,'' AS stderr")
    );
    conn.query_row(&sql, [id], row_run)
        .optional()
        .map_err(|e| e.to_string())
}
pub fn runs(
    conn: &Connection,
    task_id: Option<&str>,
    detail: Option<&str>,
    offset: u32,
) -> Result<Vec<Run>> {
    let projection = if detail.is_none() {
        COLUMNS.replace("stdout,stderr", "'' AS stdout,'' AS stderr")
    } else {
        COLUMNS.into()
    };
    let sql=format!("SELECT {projection} FROM timer_runs WHERE (?1 IS NULL OR task_id=?1) AND (?2 IS NULL OR id=?2) ORDER BY started_at DESC,rowid DESC LIMIT 100 OFFSET ?3");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![task_id, detail, offset], row_run)
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}
pub fn reserve(
    conn: &Connection,
    task: &Task,
    source: &str,
    at: &str,
    status: &str,
    message: &str,
) -> Result<Option<String>> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let finished = if status == "running" {
        None
    } else {
        Some(now.clone())
    };
    let n=conn.execute("INSERT OR IGNORE INTO timer_runs(id,task_id,revision,source,scheduled_at,started_at,finished_at,status,message,snapshot_json) VALUES (?,?,?,?,?,?,?,?,?,?)",params![id,task.id,task.revision,source,at,now,finished,status,message,serde_json::to_string(&task.redacted()).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    Ok(if n == 0 { None } else { Some(id) })
}
pub fn cleanup(conn: &Connection) -> Result<()> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
    conn.execute(
        "DELETE FROM timer_runs WHERE status!='running' AND finished_at<?",
        [cutoff],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM timer_runs WHERE status!='running' AND id NOT IN (SELECT id FROM timer_runs ORDER BY started_at DESC,rowid DESC LIMIT 10000)",[]).map_err(|e|e.to_string())?;
    // Bound aggregate retained output (~200 MiB), oldest finished runs first. Definitions/snapshots remain.
    conn.execute("UPDATE timer_runs SET stdout='',stderr='',truncated=1 WHERE status!='running' AND id IN (SELECT id FROM (SELECT id,SUM(length(CAST(stdout AS BLOB))+length(CAST(stderr AS BLOB))) OVER (ORDER BY started_at DESC,rowid DESC) AS total FROM timer_runs) WHERE total>209715200)",[]).map_err(|e|e.to_string())?;
    Ok(())
}
