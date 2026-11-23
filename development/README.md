# 开发说明

## 结构

- `apps/desktop/src`：桌面前端。`app` 管理工作区和视图生命周期，`tasks` 管理任务界面，`plugins` 管理插件界面及共享类型。
- `apps/desktop/src-tauri/src`：本机命令、托盘和退出流程。阻塞命令统一通过 `blocking` 模块进入后台线程。
- `crates/process`：普通子进程启动、进程树停止和输出解码；定时器、诊断插件和同步插件共用。
- `crates/scheduler`：Cron、SQLite、执行状态与调度；不包含内置 Bash 或历史提醒表迁移。
- `crates/plugin-host`：API 2 插件安装和诊断，以及声明式同步工作流；不执行插件提供的任意 Shell 代码。
- `examples`：可选业务示例和无业务写入的诊断脚本，不是应用运行依赖。

## 检查

使用 rustup stable，避免旧版系统 Rust 覆盖工具链。以下命令从仓库根目录执行：

```sh
npm --prefix development/apps/desktop ci
npm --prefix development/apps/desktop run format:check
npm --prefix development/apps/desktop test
npm --prefix development/apps/desktop run build

cargo test --manifest-path development/crates/process/Cargo.toml --locked
cargo test --manifest-path development/crates/scheduler/Cargo.toml --locked
HANDYKIT_REQUIRE_SCM_TESTS=1 cargo test --manifest-path development/crates/plugin-host/Cargo.toml --locked

cargo clippy --manifest-path development/crates/process/Cargo.toml --all-targets --locked -- -D warnings
cargo clippy --manifest-path development/crates/scheduler/Cargo.toml --all-targets --locked -- -D warnings
cargo clippy --manifest-path development/crates/plugin-host/Cargo.toml --all-targets --locked -- -D warnings
python3 -m unittest discover -s scripts/tests -v
python3 scripts/check_design_system.py
node --test scripts/test_appearance.cjs
python3 scripts/check_version.py
```

SCM 回归需要 git、svn、svnadmin，且只操作临时本地仓库。Windows 专用测试验证所选 Git Bash 的中文路径、实时输出与退出码；交叉编译检查不能代替 Windows/沙箱实测。

前端使用锁定版本 Prettier，Rust 使用 rustfmt。格式化命令为 `npm --prefix development/apps/desktop run format` 和各 Rust manifest 的 `cargo fmt`。TypeScript 启用 strict、noUnusedLocals、noUnusedParameters，CI 将 Clippy 警告视为失败。

## 运行约束

先写入执行记录并登记控制器，再启动动作；取消和超时不回滚已完成的外部操作。Windows 不使用自建 Job 或 breakaway，进程树停止为尽力操作；独立应用不被当作受管脚本停止。脚本的 stderr 保持独立通道，其中可包含 Git 进度，不以是否有 stderr 判断成败。

数据库 SQL 真源为 `database/migrations/0001_initial.sql`。0.2.0 重新建立初始基线；后续迁移仍应追加并校验，不能修改已发布的同一基线。产品运行使用 `app_data_dir/runtime`，测试始终使用临时目录。
