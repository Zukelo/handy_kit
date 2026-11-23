# Windows 原生插件普通子进程启动（0.1.12）

## 现场证据与判断

用户在 0.1.11 中确认：小工坊可见配置目录，并能读取父目录的 `.svn/wc.db` SQLite 文件头；所选 TortoiseSVN 客户端对同一路径返回 E155007，而从沙箱内 PowerShell 手动调用该客户端的 svn info 正常。

这排除了“该目录根本不存在”这一解释，定位重点转向子进程上下文。但尚未在开发机复现，不能认定 Job Object 或挂起启动已被证明是 Cloudaemon 问题根因；wc.db 可读也不代表插件自身完成了工作副本语义校验。

## 本次变更

只调整原生同步插件（手动执行、执行前检查、原生定时计划共用）的 Windows 启动器：

- 保持直接执行配置的 Git/SVN 程序及 Unicode argv，不增加 Bash、PowerShell 包装或自动重试。
- 普通启动，仅保留 CREATE_NO_WINDOW 隐藏控制台；去掉 CREATE_SUSPENDED、线程枚举/ResumeThread 以及自建 Job Object 和 AssignProcessToJobObject。
- 继承父进程现有环境与系统/沙箱进程关联，不使用 breakaway、不提升权限、不修改沙箱策略或物理路径。
- 执行前检查与运行日志显示实际启动策略；保留 0.1.11 原始路径和工作副本管理目录诊断。

通用定时器的其他动作与旧诊断插件启动器不在本次变更范围。

## 停止与限制

Windows 取消/超时在直接子进程仍运行时，通过系统目录中的 taskkill.exe /PID /T /F 请求终止进程树，等待上限 2 秒；失败则使用保留的进程句柄请求终止直接子进程，输出“进程树停止未确认”提示。系统工具路径通过 GetSystemDirectoryW 获取，不通过 PATH 查找，也不使用 Shell 拼接参数。正常退出不调用 taskkill。

这是尽力停止，无法保证已经脱离父进程的辅助程序或父进程先退出后的后代被清理；小工坊意外崩溃时也不再提供自建 Job 的 KILL_ON_JOB_CLOSE 保证。正常退出小工坊仍等待或请求停止活动同步。进程停止不回滚已完成的 Git/SVN 远端写入，不自动重试未知提交。

Windows 标准进程继承规则参考 [Microsoft Inheritance](https://learn.microsoft.com/en-us/windows/win32/procthread/inheritance)，进程树停止参数参考 [taskkill](https://learn.microsoft.com/windows-server/administration/windows-commands/taskkill)。这些文档不能证明 Cloudaemon 的具体实现行为。

## 验证与现场步骤

- 本机插件宿主 10 项测试通过，包含临时 Git/SVN 中文目录端到端同步；未访问业务远端。
- Windows 测试代码交叉检查通过，新增真实 PowerShell 后代进程取消测试，但未在 Windows 执行。
- Windows 0.1.12 在线 NSIS 与独立 exe 完成交叉构建；目标 Cloudaemon 的恢复效果仍需验证。

升级宿主后无需重装插件。保持同一份配置，先点「执行前检查」，核对日志出现“Windows 启动策略：普通子进程”。检查不 push/commit。只有检查成功且核对业务配置后才手动同步。如仍返回 E155007，提交启动策略、客户端、原始目录、管理目录及错误上下文，以便继续比较沙箱对子进程的处理。
