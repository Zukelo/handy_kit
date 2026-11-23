CREATE TABLE timer_tasks (
    id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK(revision > 0),
    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)),
    config_json TEXT NOT NULL CHECK(json_valid(config_json)),
    updated_at TEXT NOT NULL
);
CREATE TABLE timer_runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES timer_tasks(id),
    revision INTEGER NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('manual','schedule')),
    scheduled_at TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL CHECK(status IN ('running','succeeded','failed','launched','skipped','cancelled','timed_out','interrupted')),
    exit_code INTEGER,
    message TEXT NOT NULL DEFAULT '',
    stdout TEXT NOT NULL DEFAULT '',
    stderr TEXT NOT NULL DEFAULT '',
    truncated INTEGER NOT NULL DEFAULT 0,
    snapshot_json TEXT NOT NULL CHECK(json_valid(snapshot_json))
);
CREATE UNIQUE INDEX timer_run_dispatch ON timer_runs(task_id, revision, scheduled_at) WHERE source='schedule';
CREATE INDEX timer_run_history ON timer_runs(started_at DESC);
CREATE INDEX timer_run_task ON timer_runs(task_id,started_at DESC);
