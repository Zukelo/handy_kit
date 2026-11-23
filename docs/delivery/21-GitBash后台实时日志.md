# Git Bash 后台执行与实时日志（0.1.14）

## 变更

用户确认 0.1.13 普通子进程启动方式可在 Cloudaemon 中正常调用 git-bash.exe，但会打开独立窗口，脚本状态与输出无法回传。本次对 Windows 定时「脚本」动作中所选程序文件名为 git-bash.exe 的情况，启用 Git for Windows 启动器的后台命令参数：

```
git-bash.exe --no-needs-console --hide --no-cd --no-append-quote "--command=usr\bin\bash.exe --login" -- <脚本逻辑路径> <各独立参数>
```

小工坊直接创建的仍是配置的 git-bash.exe，不探测、替换或直接调用沙箱外其他 bash。由该启动器自行定位其安装目录中的 usr/bin/bash.exe，完成 MSYSTEM、PATH、HOME 等 Git Bash 初始化，启动登录、非交互 Bash 并等待退出，继承小工坊的标准输出/错误管道。继续采用普通启动，不挂起、不新建 Job、不使用 breakaway。其他解释器的调用规则保持原样；不支持此命令模式时报告错误，不自动回退到弹窗模式。

此机制依据 [Git for Windows 启动器源码](https://github.com/git-for-windows/MINGW-packages/blob/main/mingw-w64-git/git-wrapper.c) 的命令参数、句柄继承和等待退出实现；Git 安装脚本也使用[同类无控制台参数](https://github.com/git-for-windows/build-extra/blob/main/post-install.bat)。官方源码不等同于本次在目标沙箱的运行证明。

## 用户可见行为

- 不经 Mintty 打开独立终端，输出接入小工坊现有日志窗口。
- 前台脚本运行期间状态保持执行中，同一计划重复触发仍跳过；结束后保存退出码及成功/失败状态。
- 标准输出与标准错误分别实时收集，窗口每 1.5 秒刷新。停留在底部时跟随新日志，向上阅读时保留位置。
- 运行中不显示尚未完整收到的 UTF-8 尾字符；Windows 非 UTF-8 输出尝试按系统 ANSI 代码页解码。历史保存最终输出。
- 日志仍受现有总计 10 MiB 容量限制并显示截断提示；不提供 stdout/stderr 跨流的精确先后顺序。

## 边界

这是登录但非交互 Shell；依赖 read、交互提示、TTY 或仅在 .bashrc 定义内容的脚本可能需要调整。登录配置文件由 Git Bash 正常加载，用户配置文件若主动改变目录或重定向输出，会影响执行。

宿主只能收到脚本/程序写入管道的内容。自行重定向到文件、后台脱离、外部程序缓冲或 UI 交互内容不会自动变成实时日志；需要时使用程序自身的非缓冲选项。不要在日志中打印口令。取消/超时沿用 0.1.13 尽力终止进程树的机制；脚本主动后台脱离的工作不属于前台退出状态保证。

## 验证

- 调度器本机 16 项通过：新增运行中 stdout/stderr 可见、结束退出码 23、同任务防重叠和 UTF-8 分片回归。
- 前端 24 项通过：动态日志更新同一对话框、自动跟随、阅读位置保留、状态与退出码更新、HTML 转义；前端构建及设计检查通过。
- Windows 测试代码交叉检查通过，新增/更新真实 git-bash 启动器实时 stdout/stderr、MINGW、中文/空格/空参数、等待结束、退出码和历史测试。本次仅编译这些 Windows 用例，未在 Windows 执行。
- 0.1.14 Windows 在线安装包及独立 exe 构建通过；Cloudaemon 无窗口效果与 Git Bash 状态透传待用户验证。

## 用户验证

升级宿主即可，无需重建任务。先复制原任务保持暂停，将目标换成包内 live-logs.sh，保留所选 git-bash.exe 及工作目录：

1. 不填参数手动执行，应无新终端；日志逐秒出现三条进度，标准错误单独显示，结束状态成功、退出码 0。
2. 参数填一个独立的 `23` 再执行，应显示失败和退出码 23，已产生的日志仍可查询。
3. 再运行 check-context.sh 核对沙箱文件可见性，最后验证原业务脚本，避免与原生插件/旧计划同时重复提交。

两份诊断脚本不访问业务仓库。live-logs.sh 仅输出合成日志和等待；check-context.sh 会在自身目录写入上下文报告。
