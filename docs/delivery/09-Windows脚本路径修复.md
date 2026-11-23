# Windows sh 脚本路径修复（0.1.2）

针对用户反馈 Bash 打开 `D:/bsoft/project/01/sync_code/sync_code-merge.sh` 时提示 No such file or directory，补齐 Windows 到 MSYS 路径处理。该错误只能确认 Bash 未能打开该路径，不据此认定用户文件缺失或脚本内容错误。

0.1.2 在执行时使用所选 shell 同目录的 `cygpath.exe -u -- <Windows路径>` 获取对应安装的 POSIX 路径，避免仅将反斜杠替换为正斜杠。如果解释器目录没有 cygpath，按 MSYS 盘符形式转换，例如 `D:\bsoft\...` 转为 `/d/bsoft/...`；UNC 路径保持 `//server/share/...`。

实际执行使用 `bash.exe -- <POSIX脚本路径> <逐项参数>`，不拼 shell 命令，不重写参数或脚本文件，不变更用户工作目录策略。执行消息记录实际解释器和转换后的脚本路径。转换工具失败时返回明确错误；保存与实际执行前仍检查 Windows 目标文件。

Windows 回归补充真实 D 盘脚本（路径带空格）的执行、非零退出码、stdout/stderr 保留；原 Git Bash、中文路径与空参数测试继续运行。Windows GUI 子系统修复保留。

升级前从旧版托盘彻底退出，再运行 0.1.2；原任务无需重新创建。若仍失败，需查看新版记录中的脚本路径、退出码、错误输出，并确认 D 盘是否为本地盘、映射网络盘或其他特殊挂载；Windows 客户端实际脚本仍需用户复测。
