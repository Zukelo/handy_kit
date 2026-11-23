# 小工坊 · HandyKit

本地开发工具集，使用 Rust、Tauri 2 和 TypeScript，支持 Windows、macOS、Linux。

当前版本为 **0.2.0 全新安装基线**。已实现多标签工作区、收藏与最近使用、后台定时任务、独立插件安装及 Git → ZIP → SVN 原生同步。JSON/XML 等未实现工具明确显示占位页，不执行模拟业务。

## 当前能力

- 定时脚本严格调用配置的解释器。Windows 使用普通子进程启动；选择 `git-bash.exe` 时由该启动器初始化登录环境并后台运行，收集实时标准输出/错误及退出码。
- 程序任务支持超时、取消、防重叠、并发限制与执行历史；独立应用只记录启动请求结果。
- [Git/SVN 检查插件](plugins/code-sync-check/README.md)验证本地工具和临时仓库读写；[原生同步插件](plugins/code-sync/README.md)支持静态配置导入、原生 ZIP、分阶段记录与暂停状态的定时计划。
- 不内置 Bash，不自动替换解释器，不猜测沙箱的物理盘映射。Git/SVN 由用户的本机环境提供。

## 开发与检查

需要 Rust stable（最低 1.90）、Node.js 22.12+ 和 [Tauri 平台依赖](https://v2.tauri.app/start/prerequisites/)。

```sh
cd development/apps/desktop
npm ci
npm run dev          # 浏览器预览；不执行本机程序
npm run tauri:dev    # 桌面程序
npm test
npm run build
npm run format:check
```

Rust 模块、测试与质量检查命令见 [开发说明](development/README.md)。当前架构及审查结果见 [代码审查与清理报告](docs/delivery/22-代码审查与全新基线整理.md)。

## 数据与插件

应用在系统用户数据目录的 `runtime/` 下创建 SQLite 和插件目录。首次启动没有预设任务和插件；工作区偏好保存在当前 WebView 的本机存储。不会读取或迁移 0.1.x 旧数据布局，也不会删除旧目录。此基线不提供历史数据库和旧插件 API 的兼容分支。

插件包使用统一的 `handykit.plugin` API 2，是声明式原生能力包。安装、停用和卸载在插件管理中完成；同步配置变化会暂停关联计划。脚本与原生同步应只启用一个实际业务计划，避免重复操作。

开发库初始化：`python3 scripts/init_database.py`。真实运行数据、凭据和构建产物不提交版本管理。

## 构建

```sh
cd development/apps/desktop
npm run build:online   # Windows 在线安装包
npm run build:offline  # Windows WebView2 离线安装包（按需）
npm run package:local  # 当前平台开发包
```

Windows 本机交叉交付默认只生成在线安装包与独立 exe。macOS/Linux 使用 `npm run tauri:build`。依赖锁定在 package-lock.json 和各 Rust 工程的 Cargo.lock；格式与静态检查在 CI 中执行。

[文档入口](docs/README.md)包含当前规范及历史设计记录；静态原型位于 `docs/prototypes/`，与实际运行代码分开。
