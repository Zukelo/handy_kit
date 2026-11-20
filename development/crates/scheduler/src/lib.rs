pub mod cron;
mod executor;
pub mod model;
mod storage;
use chrono::{DateTime, TimeZone, Utc};
use fs2::FileExt;
use model::*;
use rusqlite::{params, Connection};
use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Mutex},
};
use storage::Result;
const CONCURRENCY: usize = 3;
struct Inner {
    db: Connection,
    running: HashMap<String, (String, Arc<executor::Control>)>,
    stopping: bool,
    cursor: i64,
    notice: String,
}
struct Shared {
    inner: Mutex<Inner>,
    _owner: File,
    pub directory: PathBuf,
}
#[derive(Clone)]
pub struct Scheduler(Arc<Shared>);
impl Scheduler {
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let owner = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("scheduler.lock"))
            .map_err(|e| e.to_string())?;
        owner
            .try_lock_exclusive()
            .map_err(|_| "小工坊已在此用户目录运行，请从菜单栏/托盘显示已有窗口".to_string())?;
        let path = directory.join("handykit.sqlite3");
        let mut db = Connection::open(&path).map_err(|e| e.to_string())?;
        storage::migrate(&mut db, &path)?;
        storage::cleanup(&db)?;
        let cursor = Utc::now().timestamp() / 60 * 60;
        Ok(Self(Arc::new(Shared {
            inner: Mutex::new(Inner {
                db,
                running: HashMap::new(),
                stopping: false,
                cursor,
                notice: "关闭窗口后继续调度；彻底退出、关机和休眠期间不执行；遗漏不补跑".into(),
            }),
            _owner: owner,
            directory: directory.into(),
        })))
    }
    pub fn start(&self) {
        let weak = Arc::downgrade(&self.0);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let Some(shared) = weak.upgrade() else {
                break;
            };
            let s = Self(shared);
            if s.0.inner.lock().unwrap().stopping {
                break;
            }
            if let Err(e) = s.tick(Utc::now()) {
                s.0.inner.lock().unwrap().notice = format!("调度错误：{e}");
            }
        });
    }
    pub fn directory(&self) -> &Path {
        &self.0.directory
    }
    pub fn overview(&self) -> Result<Overview> {
        let inner = self.0.inner.lock().map_err(|e| e.to_string())?;
        let mut items = Vec::new();
        for task in storage::tasks(&inner.db)? {
            let issue = executor::validate(&task).err();
            let next = if task.enabled {
                cron::Cron::parse(&task.cron).ok().and_then(|c| {
                    task.zone
                        .parse()
                        .ok()
                        .and_then(|tz| c.next(tz, Utc::now(), 1).first().map(|t| t.to_rfc3339()))
                })
            } else {
                None
            };
            let last = storage::latest(&inner.db, &task.id)?;
            items.push(TaskItem {
                task,
                next,
                last,
                issue,
            });
        }
        Ok(Overview {
            tasks: items,
            running: inner.running.len(),
            concurrency: CONCURRENCY,
            stopping: inner.stopping,
            notice: inner.notice.clone(),
        })
    }
    pub fn save(&self, mut task: Task) -> Result<Task> {
        task.name = task.name.trim().into();
        if task.name.is_empty() || task.name.len() > 240 || task.description.len() > 4000 {
            return Err("请填写任务名称（最多 240 字节），说明最多 4000 字节".into());
        }
        let preview = cron::preview(&task.cron, &task.zone, Utc::now())?;
        task.cron = preview.expression;
        executor::validate(&task)?;
        if serde_json::to_vec(&task).map_err(|e| e.to_string())?.len() > 512 * 1024 {
            return Err("任务配置超过 512 KiB".into());
        }
        let inner = self.0.inner.lock().map_err(|e| e.to_string())?;
        if inner.stopping {
            return Err("正在退出，不能保存任务".into());
        }
        if task.id.is_empty() {
            if storage::tasks(&inner.db)?.len() >= 500 {
                return Err("首版最多保存 500 项任务".into());
            }
            task.id = uuid::Uuid::new_v4().to_string();
            task.revision = 1;
        } else {
            let old = storage::task(&inner.db, &task.id)?;
            if old.revision != task.revision {
                return Err("任务已被修改，请重新载入后编辑".into());
            }
            task.revision += 1;
        }
        inner.db.execute("INSERT INTO timer_tasks(id,revision,name,enabled,config_json,updated_at) VALUES (?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,name=excluded.name,enabled=excluded.enabled,config_json=excluded.config_json,updated_at=excluded.updated_at",params![task.id,task.revision,task.name,task.enabled,serde_json::to_string(&task).map_err(|e|e.to_string())?,Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
        Ok(task)
    }
    pub fn toggle(&self, id: &str, enabled: bool) -> Result<Task> {
        let mut task = {
            let inner = self.0.inner.lock().unwrap();
            storage::task(&inner.db, id)?
        };
        task.enabled = enabled;
        // Pausing must remain possible after target files disappear.
        if !enabled {
            let inner = self.0.inner.lock().unwrap();
            task.revision += 1;
            let updated=inner.db.execute("UPDATE timer_tasks SET revision=?,enabled=0,config_json=?,updated_at=? WHERE id=? AND revision=? AND deleted=0",params![task.revision,serde_json::to_string(&task).unwrap(),Utc::now().to_rfc3339(),id,task.revision-1]).map_err(|e|e.to_string())?;
            if updated != 1 {
                return Err("任务已被修改，请重新载入".into());
            }
            Ok(task)
        } else {
            self.save(task)
        }
    }
    pub fn delete(&self, id: &str) -> Result<()> {
        let inner = self.0.inner.lock().unwrap();
        let mut task = storage::task(&inner.db, id)?;
        task.enabled = false;
        inner
            .db
            .execute(
                "UPDATE timer_tasks SET deleted=1,enabled=0,config_json=? WHERE id=?",
                params![serde_json::to_string(&task).unwrap(), id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn history(&self, task_id: Option<&str>, offset: u32) -> Result<Vec<Run>> {
        let inner = self.0.inner.lock().unwrap();
        storage::runs(&inner.db, task_id, None, offset)
    }
    pub fn run_detail(&self, id: &str) -> Result<Run> {
        let inner = self.0.inner.lock().unwrap();
        let mut run = storage::runs(&inner.db, None, Some(id), 0)?
            .into_iter()
            .next()
            .ok_or("记录不存在")?;
        if let Some((_, ctrl)) = inner.running.get(id) {
            let o = ctrl.output.lock().unwrap();
            run.stdout = String::from_utf8_lossy(&o.stdout).into();
            run.stderr = String::from_utf8_lossy(&o.stderr).into();
            run.truncated = o.truncated;
        }
        Ok(run)
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let inner = self.0.inner.lock().unwrap();
        let (_, ctrl) = inner.running.get(id).ok_or("任务已结束")?;
        let task = storage::runs(&inner.db, None, Some(id), 0)?
            .into_iter()
            .next()
            .ok_or("记录不存在")?;
        if task.snapshot.action.kind == "application" {
            return Err("独立应用启动请求不支持取消，不会关闭已打开的应用".into());
        }
        ctrl.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub fn run_now(&self, id: &str) -> Result<String> {
        self.dispatch(id, "manual", Utc::now(), None)?
            .ok_or("记录已存在".into())
    }
    fn dispatch(
        &self,
        id: &str,
        source: &str,
        at: DateTime<Utc>,
        expected: Option<i64>,
    ) -> Result<Option<String>> {
        let mut inner = self.0.inner.lock().unwrap();
        if inner.stopping {
            return Err("正在退出，已停止接收新任务".into());
        }
        let task = storage::task(&inner.db, id)?;
        if source == "schedule" && (!task.enabled || expected != Some(task.revision)) {
            return Ok(None);
        }
        let reason = if inner.running.values().any(|(task_id, _)| task_id == id) {
            Some("上一轮仍在运行，已按重叠策略跳过")
        } else if inner.running.len() >= CONCURRENCY {
            Some("已达到全局并发上限 3，本次跳过")
        } else {
            None
        };
        let run_id = storage::reserve(
            &inner.db,
            &task,
            source,
            &at.to_rfc3339(),
            if reason.is_some() {
                "skipped"
            } else {
                "running"
            },
            reason.unwrap_or("正在启动"),
        )?;
        let Some(run_id) = run_id else {
            return Ok(None);
        };
        if reason.is_some() {
            return Ok(Some(run_id));
        }
        let ctrl = Arc::new(executor::Control::default());
        inner
            .running
            .insert(run_id.clone(), (id.into(), ctrl.clone()));
        drop(inner);
        let service = self.clone();
        let run_id_copy = run_id.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                executor::execute(&task, ctrl.clone())
            }))
            .unwrap_or(executor::Outcome {
                status: "interrupted".into(),
                code: None,
                message: "执行器异常，结果未知".into(),
            });
            let mut inner = service.0.inner.lock().unwrap();
            let o = ctrl.output.lock().unwrap();
            let written=inner.db.execute("UPDATE timer_runs SET status=?,exit_code=?,finished_at=?,message=?,stdout=?,stderr=?,truncated=? WHERE id=?",params![result.status,result.code,Utc::now().to_rfc3339(),result.message,String::from_utf8_lossy(&o.stdout),String::from_utf8_lossy(&o.stderr),o.truncated,run_id_copy]);
            if let Err(e) = written {
                inner.notice = format!("执行结果保存失败：{e}，重启后将标为中断");
            }
            inner.running.remove(&run_id_copy);
            if let Err(e) = storage::cleanup(&inner.db) {
                inner.notice = format!("日志清理失败：{e}");
            }
        });
        Ok(Some(run_id))
    }
    pub fn tick(&self, now: DateTime<Utc>) -> Result<()> {
        let minute = now.timestamp() / 60 * 60;
        let tasks = {
            let mut inner = self.0.inner.lock().unwrap();
            if inner.stopping || minute <= inner.cursor {
                return Ok(());
            }
            let gap = minute - inner.cursor;
            inner.cursor = minute;
            if gap > 60 {
                inner.notice = format!(
                    "检测到 {} 分钟的调度间隔（休眠或时钟变化）；遗漏计划已跳过，不补跑",
                    gap / 60
                );
            }
            let late = now.timestamp() - minute > 30;
            let mut ready = Vec::new();
            for task in storage::tasks(&inner.db)?.into_iter().filter(|t| t.enabled) {
                let c = cron::Cron::parse(&task.cron)?;
                let tz = task.zone.parse().map_err(|_| "已保存任务时区无效")?;
                if c.matches(Utc.timestamp_opt(minute, 0).unwrap(), tz) {
                    if late {
                        storage::reserve(
                            &inner.db,
                            &task,
                            "schedule",
                            &Utc.timestamp_opt(minute, 0).unwrap().to_rfc3339(),
                            "skipped",
                            "调度延迟超过 30 秒宽限，本次跳过",
                        )?;
                    } else {
                        ready.push((task.id, task.revision));
                    }
                }
            }
            ready
        };
        for (id, revision) in tasks {
            self.dispatch(
                &id,
                "schedule",
                Utc.timestamp_opt(minute, 0).unwrap(),
                Some(revision),
            )?;
        }
        Ok(())
    }
    pub fn running_count(&self) -> usize {
        self.0.inner.lock().unwrap().running.len()
    }
    pub fn shutdown(&self, stop: bool) {
        let mut inner = self.0.inner.lock().unwrap();
        inner.stopping = true;
        if stop {
            for (_, ctrl) in inner.running.values() {
                ctrl.cancel.store(true, Ordering::SeqCst);
            }
        }
    }
}

#[cfg(test)]
mod tests;
