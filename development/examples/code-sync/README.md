# 代码同步脚本诊断版

文件：`sync_code-merge-debug.sh`。基于用户提供的 Git 合并/推送、ZIP 归档及 SVN 提交脚本增加日志；**正常模式仍会 git push 和 svn commit**。本次未连接任何真实 Git/SVN 仓库。

## 放置和运行

1. 保留原脚本作备份，将诊断版复制到 `D:\bsoft\project\01\sync_code`，与原 `config.sh`、`repository.ini` 放在同一目录。
2. 先在小工坊任务中选择这个诊断脚本，保留现有 Bash 解释器，添加一个独立参数 `--check`，手动执行。它会加载配置和检查工具，不执行同步；`config.sh` 本身是 shell 代码，会被 source。
3. 确认检查通过后，移除 `--check` 再执行真实同步。
4. stdout/stderr 会实时显示在小工坊日志中，并写入脚本目录的 `sync_code-debug-时间-PID.log`，每次运行独立保存。

也可在 Git Bash 中运行：

```bash
bash ./sync_code-merge-debug.sh --check
# 以下命令会执行原来的远程推送/提交操作
bash ./sync_code-merge-debug.sh
```

需要 Bash 4.2+、Git、zip、svn 和 Git Bash 常见基础工具；不再执行 `read -p`，无终端输入也不会等待回车。请勿为获得详细日志额外启用 `bash -x`；脚本已提供分阶段日志并在加载配置后关闭意外开启的 xtrace。

## 日志内容

- 加载配置前就建立日志，记录启动目录、脚本位置、Bash/系统信息、TTY 状态和 PATH。
- 配置文件的可读性、source 退出码、配置错误所在文件/行号/函数栈、source 后的目录以及恢复后的工作目录。
- 工具的实际位置与版本；配置项只记录是否存在，密码不打印；提示配置文件首行的 BOM/CRLF，不自动重写源文件；含 BOM 的可执行 config.sh 会在 source 前停止并说明原因。
- 仓库节、分支、合并分支、克隆目录；URL 仅标记“已配置”。
- clone/fetch/merge/push/HEAD、压缩、复制、SVN commit、清理各阶段的开始、实时 stdout/stderr、退出码、耗时与工作目录。
- 最终成功/失败数量、SVN 提交数量、总耗时及最终退出码。发生信号或异常退出也尽量写结束记录；SIGKILL/断电无法保证最后一条日志。

不输出原始命令参数、配置全文或 BASH_COMMAND；对已知密码/token 值、HTTP/SSH/SVN URL 中的用户信息及 HTTP 查询串做脱敏。配置加载的输出先临时缓冲，加载完成后再脱敏输出，临时目录仅当前用户可访问并在退出时清理。任意外部程序仍可能输出无法识别的敏感信息，共享日志前请检查。SVN 密码仍按原脚本的 CLI 参数方式使用，日志脱敏不等于凭据加密或系统进程参数隐藏。

## 同时修正的诊断相关问题

- `config.sh` 和 `repository.ini` 按脚本目录定位；source 改变工作目录时会记录并恢复。日志不再等到 source 成功后才初始化。
- 每个 repository 节读取完整后才执行，防止 directory 在 mergebranch 前出现时过早触发克隆；接受 INI 的 CRLF 和开头 BOM。
- fetch/merge/push/cd/cp/清理失败都显式记录并传播，不再失败后继续并显示“全部成功”。
- 路径全部引用，支持普通含空格目录。
- mergebranch 留空会明确记录并跳过合并；有值时任何 fetch/merge 失败都停止当前仓库。
- 只提交本次成功生成的 ZIP，不扫描/删除目录内其他 ZIP；失败时保留克隆目录或归档。已有同名目录/ZIP 时停止该仓库，供人工检查和备份，不自动覆盖清理。
- 克隆目录限制为脚本目录下的相对路径，清理前核对实际目录，拒绝越界删除。
- 不自动 svn add、svn update 或处理冲突；这些行为不在原脚本中。SVN 目标应是已有工作副本，待提交 ZIP 应已纳入版本控制，否则会记录具体失败。

`config.sh` 使用原来的变量名：`svn_dir`、`svn_username`、`svn_password`、`svn_timeout`（正整数秒）。`repository.ini` 使用 `[repository...]`、`branch`、`mergebranch`、`url`、`directory`。日志和临时文件使用私有权限，原脚本、配置及源码文件不会被诊断模式重写。

## 验证

```sh
bash -n development/examples/code-sync/sync_code-merge-debug.sh
python3 -m unittest discover -s scripts/tests -p test_sync_diagnostics.py -v
```

测试仅使用临时目录和模拟 git/zip/svn，覆盖含空格路径、完整配置节、CRLF/BOM、命令输出和退出码、merge 失败后的停止、SVN 失败保留归档、检查模式、配置错误行号及密码/URL/xtrace 脱敏；不会实际克隆、推送或提交。
