# Windows 内置 Bash 运行时（0.1.7）

日期：2026-11-22。

> 此方案经用户在 Cloudaemon 内验证无效，已于 0.1.8 撤回。本文仅保留实验记录，准备脚本、运行时配置及当前界面选项均已移除。请勿再按本文构建步骤操作。参见 [精简回退与脚本替代方案](15-精简回退与脚本替代方案.md)。

## 使用方式

Windows 脚本任务的「执行动作」页新增解释器选择：内置 Bash / 自定义解释器。新任务默认内置 Bash；已有任务原样保留，编辑原任务并切换后保存才会使用内置模式。选择内置 Bash 后无需再选择解释器文件。

内置模式是随应用携带的 Windows Bash 进程，不是将 Linux Shell 指令重新实现为 JavaScript，也不是将 Bash 加载到小工坊主进程中。它直接使用安装资源目录的 `runtimes/git-bash/usr/bin/bash.exe`，跳过外部 `git-bash.exe`、Mintty、用户安装目录和启动器自己的路径查找。定时触发与手动执行共用同一后台入口。

本次是针对 Cloudaemon 的实验性对照版本。Bash 仍属于子进程，不能保证绕过或自动修复第三方沙箱的文件视图继承；未对接 Cloudaemon 私有 API，也不寻找其底层工作盘。

## 运行行为

- 持久化使用 `builtin:bash` 标记，运行时从 Tauri 资源目录解析位置。安装路径变化后不依赖旧安装目录；任务格式沿用已有 JSON，无数据库迁移。
- 仅 Windows x64 提供内置运行时；其他平台明确报错，不偷偷替换为系统 Shell。自定义解释器继续遵循 0.1.6 的逻辑路径和环境继承规则。
- 内置 Bash 使用 `--noprofile --norc -- <脚本逻辑路径> <逐项参数>`。不加载用户交互启动文件；默认移除继承的 BASH_ENV、ENV、PWD，设置 MSYSTEM=MINGW64，PATH 优先使用运行时 usr/bin、mingw64/bin、cmd，再保留继承的 PATH。HOME 已存在时保留，不存在时使用 USERPROFILE（如有）。任务显式环境变量覆盖最终优先，包括 PATH 和 BASH_ENV。
- 配套提供 PortableGit 中的 Git、Bash 和基础工具，不自动安装脚本涉及的 SVN、ZIP 或其他业务依赖；需按脚本要求检查并配置。继承 PATH 中的外部工具仍可能被调用。
- 工作目录与脚本路径保留逻辑盘符，参数独立传递。沿用 Job 管理、stdout/stderr 分离、取消和超时处理，避免终端窗口闪退导致错误无处查看。
- 运行时文件缺失时保存/运行报告明确错误，不回退到外部 Git；执行记录注明内置模式和实际解释器。运行时缺失不导致整个应用无法打开。

## 依赖来源与构建

运行时锁定为官方 `PortableGit-2.56.1-mingw64.7z.exe`，发布标签 `v2.56.1.windows.1`。固定 URL、版本和官方 SHA-256 位于 `scripts/bash_runtime.lock.json`。保留完整上游内容、许可证和 notices，并增加来源清单。

```sh
cd development/apps/desktop
npm run runtime:prepare
npm run build:online
```

准备脚本将已校验运行时解压到忽略提交的 `src-tauri/resources/bash-runtime/`；Windows Tauri 配置映射到安装目录 `runtimes/git-bash/`。准备时需安装 7z/7zz（macOS 可用 `brew install sevenzip`）；仅解压，不执行下载的 Windows 自解压程序。在线/离线 Windows 打包和发布流程准备依赖，CI Windows 平台增加实际 Bash 执行回归。

本地开发包仍仅生成 WebView2 在线安装方式。Bash 文件已经随包附带，目标机不需要下载 Bash；WebView2 缺失时仍需联网安装。便携交付必须包含 exe 和整个 runtimes 目录，不能只复制单个 exe。

## 验证与剩余验收

本机后端 13 项、前端 18 项测试通过，前端构建及 Windows 测试代码交叉编译通过。新增回归覆盖 Windows 新任务默认值、解释器切换与保存、旧自定义入口保留、内置运行时路径、缺失时禁止回退、环境覆盖优先级和非 Windows 明确拒绝。

Windows 实际 Bash 回归已加入 CI：完整运行时执行中文/空参数脚本、调用 basename 与 git、输出 stderr、返回非零退出码并查询持久化快照。本机为 macOS，此回归只完成编译，未在 Windows 执行。Cloudaemon 仍需用户在新版中切换原任务到内置 Bash，先手动运行再验证定时触发及仅沙箱内文件可见性。

来源：[官方发布及校验信息](https://github.com/git-for-windows/git/releases/tag/v2.56.1.windows.1)、[组件源码信息](https://github.com/git-for-windows)。
