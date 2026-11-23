mod config;
mod engine;
mod process;
mod store;
mod workflow;
use crate::{Host, Operation, Result};
pub use config::{Config, Imported, Repository};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
pub use store::Profile;
pub use workflow::{Step, Workflow};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoResult {
    pub name: String,
    pub directory: String,
    pub status: String,
    pub git_push: String,
    #[serde(default)]
    pub git_commit: String,
    pub svn_commit: String,
    pub archive: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub plugin_id: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub logs: Vec<String>,
    pub repositories: Vec<RepoResult>,
    pub message: String,
    pub config_revision: u64,
    #[serde(default)]
    pub plugin_version: String,
    #[serde(default)]
    pub workflow: Option<Workflow>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Preflight {
    pub passed: bool,
    pub checks: Vec<crate::Check>,
}
pub type LogSink = Arc<dyn Fn(&str) + Send + Sync>;
pub struct Control {
    pub cancel: AtomicBool,
    pub state: Mutex<Run>,
    pub sink: Option<LogSink>,
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
impl Control {
    pub fn log(&self, s: &str) {
        let mut r = self.state.lock().unwrap();
        let used: usize = r.logs.iter().map(String::len).sum();
        if used >= 256 * 1024 || r.logs.len() >= 1500 {
            return;
        }
        let mut end = s.len().min((256 * 1024 - used).min(8192));
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        let line = format!(
            "{} {}{}",
            now(),
            &s[..end],
            if end < s.len() {
                " [输出已截断]"
            } else {
                ""
            }
        );
        r.logs.push(line.clone());
        drop(r);
        if let Some(sink) = &self.sink {
            sink(&line);
        }
    }
    fn check(&self, external: Option<&AtomicBool>) -> Result<()> {
        if self.cancel.load(Ordering::SeqCst) || external.is_some_and(|c| c.load(Ordering::SeqCst))
        {
            Err("执行已取消；请检查已发生的 Git/SVN 写入，不自动回滚".into())
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests;
