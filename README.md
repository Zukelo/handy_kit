# 小工坊 · HandyKit

个人开发工具集，采用 Rust + Tauri 2，目标平台为 Windows、macOS、Linux。

## 目录

- `docs/`：需求、设计、原型、数据库设计、决策及交付文档。
- `development/`：源码与构建配置。
- `database/`：SQL 迁移、测试种子、开发数据和备份；真实数据库不提交。
- `scripts/`：初始化与校验脚本。

从 [文档入口](docs/README.md) 开始。当前交付为工程骨架、设计基线与 GUI 交互原型，不代表全部工具已实现。

直接打开 [GUI 交互原型](docs/prototypes/index.html) 可查看首页、分类菜单与多标签工作区，无需安装依赖。

## 开发

安装 Rust stable、Node.js 22+ 及 [Tauri 平台依赖](https://v2.tauri.app/start/prerequisites/)。Rust 最低版本为 1.90，建议通过 rustup 使用 stable；若与 Homebrew Rust 共存，需确保 `$HOME/.cargo/bin` 优先于 Homebrew 路径。

```sh
cd development/apps/desktop
npm ci
npm run dev
npm run tauri:dev
```

`dev` 仅启动网页；`tauri:dev` 启动桌面外壳。依赖由 package-lock.json 和 Cargo.lock 锁定；Rust 使用 stable 工具链。

```sh
npm run build:online
npm run build:offline
```

上述两个命令用于 Windows 的两种安装方式，应在 Windows 构建。macOS/Linux 使用 `npm run tauri:build`，离线交付还须执行平台验收清单。

数据库开发初始化：`python3 scripts/init_database.py`。生产运行时使用系统应用数据目录，不能写入安装目录或仓库。

## 分支与发布

仓库：[Zukelo/handy_kit](https://github.com/Zukelo/handy_kit)。`develop` 用于开发，`main` 用于稳定版本。

本机开发包：`npm run build:dev`（debug 可执行文件）或 `npm run package:dev`（本机 debug 安装包）。版本标签 `v*` 由 GitHub Actions 构建 Windows、macOS 和 Linux 安装包并发布到 Releases，日常提交只做基础检查。

详见 [Git 与版本发布流程](docs/delivery/03-Git与版本发布流程.md)。
