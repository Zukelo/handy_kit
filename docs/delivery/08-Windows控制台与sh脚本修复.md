# Windows 控制台与 sh 脚本修复（0.1.1）

日期：2026-11-20。依据 Windows 手工测试反馈，修复开发包附带控制台和 Git Bash 脚本错误难以查看的问题。

## 行为变更

- Windows 主程序始终以 GUI 子系统编译，包括 debug 开发包。启动不再附带控制台；关闭主窗口仍隐藏到托盘，彻底退出使用托盘菜单。
- 脚本解释器若选择 `Git\git-bash.exe`，解析为同安装目录下的 `usr\bin\bash.exe`（优先）或 `bin\bash.exe`，避免启动 mintty 终端。选择 `Git\bin\bash.exe` / `sh.exe` 时优先采用 `usr\bin` 中的同名实际解释器。
- 脚本目标路径仅将分隔符转换为 MSYS 兼容形式；有序参数不重写，保留中文、空格、空字符串和引号。补充解释器及 Git 工具目录到子进程 PATH；用户显式 PATH 覆盖仍优先。不自动加载交互 shell profile。
- 已知终端启动器、cmd、PowerShell、WSL 不能误作为 sh 解释器保存，错误说明应选择 bash.exe/sh.exe。其他程序仍可通过「运行程序」执行。
- 受管脚本延续无窗口、Job 管理、stdout/stderr 分离捕获和持久化。手动执行会显示 App 内日志；失败记录保留退出码、错误输出及实际解释器路径，不随外部窗口消失。

推荐配置：动作「运行脚本」，目标为 `.sh`，解释器为 `C:\Program Files\Git\usr\bin\bash.exe` 或 `sh.exe`。旧任务若保存的是 git-bash.exe，会在下次执行时解析，无需改库或导入数据。脚本自身的编码、CRLF、缺少工具、业务错误不会被静默修正，应查看保留的错误输出定位。

## 验证

新增 Windows Git for Windows 集成回归：使用真实 Git 安装，覆盖 git-bash.exe / bin\bash.exe 入口、中文带空格脚本路径、独立与空参数、外部 Unix 工具 PATH、stderr、非零退出码、结束后重新查询历史。页面回归覆盖失败日志显示、转义和退出码保留。交叉编译后检查 PE 子系统为 Windows GUI，DLL 导入仅依赖系统库。

该文记录实现和验证方法；Windows 本机 UI、用户实际脚本与托盘仍需手工复测。参考 [Git for Windows 包装器说明](https://gitforwindows.org/git-wrapper.html)和 [Microsoft 进程创建标志](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)。

## 安装复测

1. 从旧版托盘选择「彻底退出小工坊」后再安装/运行新版，避免单实例机制恢复仍在运行的旧进程。
2. 启动 0.1.1，确认不再出现日志控制台，关闭主窗口后能从托盘恢复。
3. 手动执行原 sh 任务；在自动弹出的 App 日志查看结果与「错误输出」，或从执行记录重新打开。
4. 若仍失败，提供退出码、错误输出和解释器路径；这些信息可以定位脚本自身的问题，无需截取闪退终端窗口。

这是开发修复包；不创建正式发布标签。已有数据库和任务定义保留。
