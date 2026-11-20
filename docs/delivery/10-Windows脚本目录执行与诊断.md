# Windows 脚本目录执行与诊断（0.1.3）

用户确认文件在 Windows 存在且属性为普通 Archive、无链接目标，但 Bash 仍对 D:/... 与 /d/... 均报 No such file or directory。普通 D 盘 CI 通过不能视为已复现用户机器，因此本次不再将盘符替换当作唯一修复。

当工作目录留空或等于脚本所在目录时，先由 Windows File::open 检查读取权限，再通过 Windows current_dir 设置脚本目录，向 shell 传入 `-- ./文件名.sh` 和独立参数，避开对绝对盘符路径的再次解析。显式配置其他工作目录时仍保持用户工作目录，使用 cygpath 得到的绝对路径；不修改用户脚本和任务配置。

不使用 stdin/source/eval 方式执行脚本，因为这些方式会改变脚本身份、BASH_SOURCE、标准输入或命令语义。目录执行中的 $0/BASH_SOURCE 为实际相对脚本路径，dirname 能在默认工作目录找到原脚本。依赖 $0 必须是绝对字符串的脚本需自行规范化路径，不能假设所有启动器都传绝对 $0。

诊断信息增加：Windows 读取是否成功、实际解释器、Bash 接收的脚本参数、Windows 工作目录。Windows 读取失败直接显示原生 OS 错误；Bash 失败继续保留 stderr 和退出码。这个版本仍需用户实际机器验证，不能由 CI 推断特定 D 盘环境已修复。

Windows 回归覆盖 D 盘带空格文件名、空工作目录的相对执行、自定义工作目录的绝对执行、脚本按 $0 读取自身、BASH_SOURCE 与 $0 的一致性、stdout/stderr 与退出码。GUI 子系统保持不弹控制台。

本机交叉编译遵照用户偏好仅生成在线 NSIS 安装包和独立 exe，不嵌入 WebView2 离线安装器。在线安装包在缺少 WebView2 时需要联网安装；独立 exe 需要已有 WebView2。现有离线包作为旧版历史文件保留，不新生成。
