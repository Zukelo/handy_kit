use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Argument {
    pub value: String,
    #[serde(default)]
    pub secret: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub secret: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub kind: String,
    pub target: String,
    pub interpreter: String,
    pub args: Vec<Argument>,
    pub env: Vec<Environment>,
    pub cwd: String,
    pub instance: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub revision: i64,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub cron: String,
    pub zone: String,
    pub action: Action,
    pub timeout: Option<u64>,
}
impl Task {
    pub fn redacted(&self) -> Self {
        let mut t = self.clone();
        for a in &mut t.action.args {
            if a.secret {
                a.value = "〈敏感参数〉".into();
            }
        }
        for e in &mut t.action.env {
            if e.secret {
                e.value = "〈敏感值〉".into();
            }
        }
        t
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub task_id: String,
    pub revision: i64,
    pub source: String,
    pub scheduled_at: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub snapshot: Task,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub task: Task,
    pub next: Option<String>,
    pub last: Option<Run>,
    pub issue: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub tasks: Vec<TaskItem>,
    pub running: usize,
    pub concurrency: usize,
    pub stopping: bool,
    pub notice: String,
}
