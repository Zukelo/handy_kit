# Git/SVN 检查插件与原生同步边界（0.1.9）

日期：2026-11-22。用户选择独立安装、停用、卸载的插件包，并要求先确认 Git/SVN 可用，再开展原生同步。

## 已完成与验证结论

- 新增通用 Rust 插件宿主 `development/crates/plugin-host`，桌面端新增「插件管理」。`.hkp` 插件包可以独立安装、停用、启用、卸载，无需重编译宿主安装包。
- 首个插件为 `dev.handykit.code-sync-check` 0.1.0。宿主按声明的能力运行，不按该插件 ID 硬编码；不同 ID、不同能力子集有回归覆盖。源码/包见 `plugins/code-sync-check/`。
- 本机 macOS：Git 2.54.0（Apple Git-157），SVN 1.14.5；真实临时仓库 roundtrip 通过。未连接任何真实 Git/SVN 远端。
- Git 实测：init/bare、clone、fetch、commit、分支切换、非快进 merge、本地 push、远端引用与提交一致性。
- SVN 实测：svnadmin 创建仓库、file:// checkout、add、commit、revision=1、远端内容一致性。
- 检查报告明确记录实际逻辑程序路径、平台、每项结果、失败细节和剩余验证范围，不使用「可用」一词掩盖仅通过版本检查的情况。
- Windows 宿主及测试代码完成交叉编译；Windows/Cloudaemon、真实网络和认证未实测。

## 插件格式与生命周期

`.hkp` 当前是 UTF-8 JSON 清单，最大 64 KiB。固定 `format=handykit.plugin`、`apiVersion=1`，包含 ID、名称、版本、说明和 operations。拒绝未知字段、未知能力、重复能力、不兼容 API 及越界 ID；不执行清单中的任意命令或代码。

当前能力是 `workspace.read`、`git.version`、`svn.version`、`git.local-roundtrip`、`svn.local-roundtrip`。插件选择能力；宿主实现对应原生函数。新增宿主能力需要宿主升级，已有能力的独立插件可组合使用。因此它是声明式插件机制，不宣称支持任意第三方 DLL/JS 或 ABI 热加载。

安装目录为应用用户数据目录的 `plugins/<id>/`，与调度数据库分开。插件清单与启用状态持久化；相同 ID 不静默覆盖，更新时先卸载再安装。停用禁止运行，卸载仅删除该插件目录。运行与生命周期变更串行，避免执行中卸载导致状态混乱。

宿主不附带 Git、SVN 或 Bash，不修改 PATH，不提升权限，不猜测沙箱底层盘。程序路径由用户显式选择，留空时只搜索当前 PATH；保存的逻辑路径不 canonicalize。检查在后台线程运行，结果在应用内显示并可复制 JSON。

## 本地自检的操作边界

「检查版本与目录」不写仓库；版本必须是可识别 Git/SVN 格式，而非只根据退出码 0 判定。可选文件只由主进程打开一个字节，不输出内容，因此该项不能证明 Git/SVN 子进程读取了同一文件。

「本地读写自检」仅在用户选定工作目录中新建唯一临时目录。Git 使用隔离配置、临时身份与本地 bare 仓库，禁用测试提交签名和 hooks；SVN 使用临时配置和 file:// 本地仓库。成功清理本轮数据，失败保留并报告路径。没有真实服务器 URL 或认证输入，也没有任意 Shell、push 或 commit 用户仓库的入口。

每条固定工具命令限时 15 秒，输出有长度限制；不把输出管道未关闭当作 UI 无限等待的理由。此处是固定诊断操作执行器，不替代业务调度器的 Job/取消机制。

## 后续原生同步的接入边界

用户选定的原生同步方向不变。本轮是先决检查，不把 CLI 检查改名为「已实现原生同步」。后续业务插件将声明同步工作流，逐步接入宿主原生文件、ZIP、Git、SVN 能力，再对接 Scheduler。

| 验证层 | 本轮状态 | 后续要求 |
|---|---|---|
| macOS 工具版本与隔离本地读写 | 已通过 | 保留回归 |
| Windows/Cloudaemon 文件及工具视图 | 待目标环境报告 | 由当前检查插件取得，不能以交叉编译替代 |
| 真实远端协议、认证、工作副本 | 未验证 | 先只读检查，写入使用独立测试仓库 |
| 小工坊进程内 libgit2/libsvn | 未接入 | 构建/依赖/ABI、认证回调、取消及合并语义验证 |
| 原脚本 clone/fetch/merge/push → ZIP → SVN | 未实现 | 建立原生能力后作为独立同步插件交付 |

libgit2 提供可链接的 Git 能力，Subversion 提供客户端工作副本 API，但不代表 CLI 的所有配置、hooks、认证或 `recursive -X theirs` 自动等价。必须在目标环境分别确认，不能从 `git --version` / `svn --version` 推导库已可用。

## 复现

```sh
HANDYKIT_REQUIRE_SCM_TESTS=1 cargo test --manifest-path development/crates/plugin-host/Cargo.toml --locked -- --nocapture
cd development/apps/desktop
npm test
npm run tauri:dev
```

本地真实 SCM 测试需要 git、svn、svnadmin。未设置强制开关且工具缺失时，会明确打印跳过信息；设置强制开关后不允许跳过。CI 增加插件宿主测试和格式检查。

目标环境操作步骤参见 [检查插件说明](../../plugins/code-sync-check/README.md)。Windows 开发包仍为不含 Bash 的在线安装包与独立 exe；`.hkp` 另行分发，宿主默认不自动安装插件。

参考：[Git init](https://git-scm.com/docs/git-init)、[libgit2](https://libgit2.org/)、[Subversion 客户端 API](https://subversion.apache.org/docs/api/latest/group__clnt__wc.html)。
