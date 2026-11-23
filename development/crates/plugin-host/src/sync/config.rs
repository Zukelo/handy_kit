use super::*;
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Repository {
    pub name: String,
    pub branch: String,
    pub merge_branch: String,
    pub url: String,
    pub directory: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub workspace: String,
    pub svn_dir: String,
    pub git: String,
    pub svn: String,
    pub svn_username: String,
    #[serde(default)]
    pub svn_credential_file: String,
    pub git_name: String,
    pub git_email: String,
    pub svn_timeout: u64,
    pub command_timeout: u64,
    pub repositories: Vec<Repository>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            workspace: String::new(),
            svn_dir: String::new(),
            git: String::new(),
            svn: String::new(),
            svn_username: String::new(),
            svn_credential_file: String::new(),
            git_name: String::new(),
            git_email: String::new(),
            svn_timeout: 60,
            command_timeout: 300,
            repositories: vec![],
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub config: Config,
    pub password: String,
    pub warnings: Vec<String>,
}
fn scalar(raw: &str, shell: bool) -> Result<String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(String::new());
    }
    if let Some(rest) = s.strip_prefix('\'') {
        let end = rest.find('\'').map(|i| i + 1).ok_or("单引号未闭合")?;
        let tail = s[end + 1..].trim();
        if !tail.is_empty() && !tail.starts_with('#') {
            return Err("引号后含 Shell 逻辑".into());
        }
        return Ok(s[1..end].into());
    }
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest.find('"').map(|i| i + 1).ok_or("双引号未闭合")?;
        let value = &s[1..end];
        let tail = s[end + 1..].trim();
        if !tail.is_empty() && !tail.starts_with('#') {
            return Err("引号后含 Shell 逻辑".into());
        }
        if shell && (value.contains('$') || value.contains('`') || value.contains("\\\"")) {
            return Err("不执行双引号中的变量、转义或命令展开".into());
        }
        return Ok(value.into());
    }
    let value = s.split(" #").next().unwrap_or(s).trim();
    if shell
        && (value.chars().any(char::is_whitespace)
            || value.contains(['$', '`', ';', '&', '|', '(', ')', '<', '>']))
    {
        return Err("仅支持静态变量赋值，不执行 Shell 代码".into());
    }
    Ok(value.into())
}
pub fn import(directory: &Path) -> Result<Imported> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("请选择脚本所在的绝对目录".into());
    }
    let mut c = Config {
        workspace: directory.display().to_string(),
        ..Default::default()
    };
    let mut password = String::new();
    let mut warnings = Vec::new();
    let read = |name: &str| -> Result<String> {
        let p = directory.join(name);
        let size = fs::metadata(&p).map_err(|e| format!("{name}：{e}"))?.len();
        if size > 1024 * 1024 {
            return Err(format!("{name} 超过 1 MiB"));
        }
        fs::read_to_string(p)
            .map(|s| s.trim_start_matches('\u{feff}').to_string())
            .map_err(|e| format!("{name} 需为 UTF-8：{e}"))
    };
    for (i, line) in read("config.sh")?.lines().enumerate() {
        let line = line.trim().trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, raw) = line
            .split_once('=')
            .ok_or_else(|| format!("config.sh 第 {} 行不是静态赋值，请手动迁移", i + 1))?;
        let key = key.trim();
        if !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(format!("config.sh 第 {} 行变量名无效", i + 1));
        }
        let value = scalar(raw, true)
            .map_err(|e| format!("config.sh 第 {} 行：{e}（未输出原值）", i + 1))?;
        match key {
            "svn_dir" => {
                let p = PathBuf::from(value);
                c.svn_dir = if p.is_absolute() {
                    p
                } else {
                    directory.join(p)
                }
                .display()
                .to_string()
            }
            "svn_username" => c.svn_username = value,
            "svn_password" => password = value,
            "svn_timeout" => {
                c.svn_timeout = value.parse().map_err(|_| "svn_timeout 必须为正整数")?
            }
            _ => warnings.push(format!(
                "未导入变量 {key}；如影响程序路径，请在 Git/SVN 程序字段配置"
            )),
        }
    }
    let mut current: Option<Repository> = None;
    for (i, line) in read("repository.ini")?.lines().enumerate() {
        let line = line.trim().trim_end_matches('\r');
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(r) = current.take() {
                c.repositories.push(r)
            }
            if line.starts_with("[repository") {
                current = Some(Repository {
                    name: line[1..line.len() - 1].into(),
                    ..Default::default()
                })
            }
            continue;
        }
        if let Some(r) = &mut current {
            let (k, v) = line
                .split_once('=')
                .ok_or_else(|| format!("repository.ini 第 {} 行缺少等号", i + 1))?;
            let v = scalar(v, false).map_err(|e| format!("repository.ini 第 {} 行：{e}", i + 1))?;
            match k.trim() {
                "branch" => r.branch = v,
                "mergebranch" => r.merge_branch = v,
                "url" => r.url = v,
                "directory" => r.directory = v,
                _ => warnings.push(format!("忽略仓库字段 {}（第 {} 行）", k.trim(), i + 1)),
            }
        }
    }
    if let Some(r) = current.take() {
        c.repositories.push(r)
    }
    if !password.is_empty() {
        c.svn_credential_file = directory.join("config.sh").display().to_string();
        warnings.push("仅保存原 config.sh 的凭据路径；运行时静态读取 svn_password，不执行脚本、不复制口令。也可清空路径改用认证缓存".into())
    }
    c.validate()?;
    Ok(Imported {
        config: c,
        password,
        warnings,
    })
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 512 * 1024 {
            return Err("同步配置超过 512 KiB".into());
        }
        if !Path::new(&self.workspace).is_absolute() || !Path::new(&self.workspace).is_dir() {
            return Err("工作目录必须是存在的绝对目录".into());
        }
        if !Path::new(&self.svn_dir).is_absolute() || !Path::new(&self.svn_dir).is_dir() {
            return Err("SVN 工作副本目录必须是存在的绝对目录".into());
        }
        if self.command_timeout == 0
            || self.command_timeout > 86400
            || self.svn_timeout == 0
            || self.svn_timeout > 86400
        {
            return Err("超时应为 1–86400 秒".into());
        }
        if cfg!(windows) {
            for p in [&self.git, &self.svn] {
                if !p.is_empty()
                    && !Path::new(p)
                        .extension()
                        .is_some_and(|v| v.eq_ignore_ascii_case("exe"))
                {
                    return Err("Windows Git/SVN 程序请选择 exe，不使用脚本或批处理启动器".into());
                }
            }
        }
        if !self.svn_credential_file.is_empty()
            && (!Path::new(&self.svn_credential_file).is_absolute()
                || !Path::new(&self.svn_credential_file).is_file())
        {
            return Err("口令变量文件必须是存在的绝对文件路径".into());
        }
        if self.repositories.is_empty() || self.repositories.len() > 100 {
            return Err("需配置 1–100 个仓库".into());
        }
        if self.git_name.is_empty() != self.git_email.is_empty() {
            return Err("Git 提交姓名和邮箱需同时填写或同时留空".into());
        }
        for s in [
            &self.workspace,
            &self.svn_dir,
            &self.git,
            &self.svn,
            &self.svn_username,
            &self.svn_credential_file,
            &self.git_name,
            &self.git_email,
        ] {
            if s.contains(['\0', '\n', '\r']) || s.len() > 32768 {
                return Err("配置值包含控制字符或过长".into());
            }
        }
        let mut paths: Vec<String> = Vec::new();
        let mut filenames = std::collections::HashSet::new();
        let mut artifacts: Vec<String> = Vec::new();
        for r in &self.repositories {
            if r.name.len() > 240
                || r.directory.len() > 1024
                || r.branch.len() > 1024
                || r.merge_branch.len() > 1024
            {
                return Err("仓库名称、分支或相对路径过长".into());
            }
            if r.name.is_empty()
                || r.branch.is_empty()
                || r.url.is_empty()
                || r.directory.is_empty()
            {
                return Err("仓库名称、分支、URL 和克隆目录均不能为空".into());
            }
            for s in [&r.name, &r.branch, &r.merge_branch, &r.url, &r.directory] {
                if s.contains(['\0', '\n', '\r']) || s.len() > 32768 {
                    return Err("仓库配置包含控制字符或过长".into());
                }
            }
            if r.branch.starts_with('-') || r.merge_branch.starts_with('-') {
                return Err("分支名不能以 - 开头".into());
            }
            let d = &r.directory;
            if d.contains('\\')
                || d.contains(':')
                || d.starts_with('/')
                || d.ends_with('/')
                || d.split('/').any(|v| {
                    v.is_empty()
                        || v == "."
                        || v == ".."
                        || v.ends_with(['.', ' '])
                        || v.contains(['*', '?', '"', '<', '>', '|'])
                })
            {
                return Err(format!("仓库 {} 的克隆目录必须是安全的相对目录", r.name));
            }
            let key = d.to_lowercase();
            if paths.iter().any(|p| {
                p == &key || p.starts_with(&format!("{key}/")) || key.starts_with(&format!("{p}/"))
            }) {
                return Err("克隆目录不能重复或相互嵌套".into());
            }
            for candidate in [&key, &format!("{key}.zip")] {
                if artifacts.iter().any(|previous| {
                    previous == candidate
                        || previous.starts_with(&format!("{candidate}/"))
                        || candidate.starts_with(&format!("{previous}/"))
                }) {
                    return Err("克隆目录与其他仓库的 ZIP 输出路径冲突".into());
                }
                artifacts.push(candidate.clone());
            }
            paths.push(key);
            if !filenames.insert(r.filename().to_lowercase()) {
                return Err("多个仓库的 ZIP 文件名相同，无法映射到 SVN".into());
            }
            if r.url.starts_with('-') || r.url.to_ascii_lowercase().starts_with("ext::") {
                return Err("不支持该 Git URL 或外部命令协议".into());
            }
            if let Ok(url) = url::Url::parse(&r.url) {
                if url.password().is_some()
                    || url.query_pairs().any(|(k, _)| {
                        ["token", "access_token", "password", "secret"]
                            .contains(&k.to_ascii_lowercase().as_str())
                    })
                {
                    return Err("Git URL 不得内嵌口令或令牌，请使用现有凭据管理器".into());
                }
            }
            no_child_links(Path::new(&self.workspace), d)?;
        }
        Ok(())
    }
}
impl Repository {
    pub fn filename(&self) -> String {
        format!("{}.zip", self.directory.rsplit('/').next().unwrap())
    }
}
pub fn no_child_links(root: &Path, relative: &str) -> Result<()> {
    let mut p = root.to_path_buf();
    for component in relative.split('/') {
        p.push(component);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err("克隆路径包含符号链接或重解析链接，已停止".into())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

pub fn password_file(path: &str) -> Result<String> {
    let file = Path::new(path);
    if !file.is_absolute() || !file.is_file() {
        return Err("口令变量文件不存在或不是绝对文件路径".into());
    }
    if fs::metadata(file).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("口令变量文件超过 1 MiB".into());
    }
    let text = fs::read_to_string(file).map_err(|e| format!("读取口令变量文件失败：{e}"))?;
    let mut password = None;
    for (i, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("口令变量文件第 {} 行不是静态赋值；不会执行脚本", i + 1))?;
        let value = scalar(value, true)
            .map_err(|_| format!("口令变量文件第 {} 行包含动态 Shell 语法；不会执行", i + 1))?;
        if key.trim() == "svn_password" {
            password = Some(value);
        }
    }
    let password = password.ok_or("口令变量文件缺少静态 svn_password 赋值")?;
    if password.len() > 1024 || password.contains(['\0', '\r', '\n']) {
        return Err("口令变量无效或过长".into());
    }
    Ok(password)
}
