#!/usr/bin/env bash
# Git -> ZIP -> SVN 同步诊断版。--check 只加载配置并检查本机环境，不执行同步。
# 正常模式保留原脚本的 git merge / git push / svn commit 行为。

if (( BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 2) )); then
  printf '需要 Bash 4.2+，请使用 Git for Windows 的 bash.exe。\n' >&2
  exit 2
fi
set +e +u +x
set -E -o pipefail
umask 077
HK_CHECK_ONLY=0
case "${1:-}" in
  --check) HK_CHECK_ONLY=1 ;;
  -h|--help)
    printf '用法：bash sync_code-merge-debug.sh [--check]\n配置：脚本同目录 config.sh、repository.ini。正常模式会向 Git/SVN 提交。\n'
    exit 0 ;;
  '') ;;
  *) printf '未知参数；使用 --help 查看用法。\n' >&2; exit 2 ;;
esac

HK_START_CWD=$PWD
HK_SCRIPT_SOURCE=${BASH_SOURCE[0]}
HK_SCRIPT_DIR=$(cd -- "$(dirname -- "$HK_SCRIPT_SOURCE")" && pwd -P) || {
  printf '无法确定脚本目录，source=%s cwd=%s\n' "$HK_SCRIPT_SOURCE" "$HK_START_CWD" >&2
  exit 2
}
printf -v HK_RUN_ID '%(%Y%m%d-%H%M%S)T-%s' -1 "$$"
HK_LOG_FILE="$HK_SCRIPT_DIR/sync_code-debug-$HK_RUN_ID.log"
: >> "$HK_LOG_FILE" || { printf '无法写日志：%s\n' "$HK_LOG_FILE" >&2; exit 2; }
HK_SED=$(command -v sed || true)
if [[ -z "$HK_SED" ]]; then
  printf '缺少 sed，无法可靠脱敏日志，停止。\n' | command tee -a "$HK_LOG_FILE" >&2
  exit 2
fi
HK_TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/handykit-sync.XXXXXXXX") || exit 2
HK_PHASE=bootstrap
HK_CONFIG_ACTIVE=0
HK_CONFIG_ERRORS=0
HK_CONFIG_PENDING=0
HK_SECTIONS=0
HK_SUCCEEDED=0
HK_FAILED=0
HK_COMMITTED=0
HK_START_SECONDS=$SECONDS
HK_ARCHIVES=()

# 不启用 set -x、不打印配置全文或原始 argv。已知凭据及 HTTP URL 凭据/查询串脱敏。
hk_redact() {
  local text=$1 key value
  for key in svn_password git_password password token git_token access_token; do
    value=${!key-}
    if [[ -n "$value" ]]; then text=${text//"$value"/[REDACTED]}; fi
  done
  if [[ -n "$HK_SED" ]]; then
    printf '%s' "$text" | "$HK_SED" -E \
      -e 's#((https?|ssh|svn|svn\+ssh|git)://)[^/@[:space:]]+@#\1[REDACTED]@#g' \
      -e 's#(https?://[^?[:space:]]+)\?[^[:space:]]*#\1?[REDACTED]#g' \
      -e 's#([A-Za-z_]*([Pp][Aa][Ss][Ss][Ww][Oo][Rr][Dd]|[Tt][Oo][Kk][Ee][Nn]|[Ss][Ee][Cc][Rr][Ee][Tt])[A-Za-z_]*[[:space:]]*[=:]).*#\1[REDACTED]#g'
  else
    # 引导阶段缺少 sed 时不显示任意命令输出，避免无 URL 脱敏的日志。
    printf '[输出暂不显示：sed 不可用]'
  fi
}
hk_log() {
  local level=$1 text stamp line
  shift
  text=$(hk_redact "$*")
  printf -v stamp '%(%Y-%m-%dT%H:%M:%S%z)T' -1
  printf -v line '%s [%s] [run=%s pid=%s phase=%s] %s' "$stamp" "$level" "$HK_RUN_ID" "$BASHPID" "$HK_PHASE" "$text"
  if ! printf '%s\n' "$line" >> "$HK_LOG_FILE"; then
    printf '日志写入失败：%s\n' "$HK_LOG_FILE" >&2
    return 74
  fi
  case "$level" in ERROR|WARN|CMD-ERR) printf '%s\n' "$line" >&2 ;; *) printf '%s\n' "$line" ;; esac
}
hk_stream() {
  local kind=$1 label=$2 text
  while IFS= read -r text || [[ -n "$text" ]]; do
    hk_log "$kind" "$label | $text" || return 74
  done
  return 0
}
hk_probe_encoding() {
  local file=$1 label=$2 first=''
  if IFS= read -r first < "$file"; then :; else :; fi
  if [[ "$first" == $'\xEF\xBB\xBF'* ]]; then
    hk_log WARN "$label 首行包含 UTF-8 BOM（不自动修改源文件）"
    if [[ "$label" == config.sh ]]; then hk_log ERROR '停止 source 含 BOM 的 shell 配置，请先另存为 UTF-8 无 BOM。'; return 65; fi
  fi
  if [[ "$first" == *$'\r' ]]; then hk_log WARN "$label 首行使用 CRLF，执行型 shell 配置可能受影响（不打印内容）"; fi
}
hk_flush_config() {
  if (( HK_CONFIG_PENDING )); then
    HK_CONFIG_PENDING=0
    if [[ -f "$HK_TEMP_DIR/config.out" ]]; then hk_stream CONFIG-OUT config.sh < "$HK_TEMP_DIR/config.out"; fi
    if [[ -f "$HK_TEMP_DIR/config.err" ]]; then hk_stream CMD-ERR config.sh < "$HK_TEMP_DIR/config.err"; fi
    rm -f -- "$HK_TEMP_DIR/config.out" "$HK_TEMP_DIR/config.err"
  fi
}
hk_on_error() {
  local rc=$1 line=$2 source=$3 stack=$4
  set +x
  if (( HK_CONFIG_ACTIVE )); then HK_CONFIG_ERRORS=$((HK_CONFIG_ERRORS + 1)); fi
  # 不输出 BASH_COMMAND，它可能含密码或配置中的字面量凭据。
  hk_log ERROR "未处理命令失败：rc=$rc source=$source line=$line stack=$stack" || true
  return 0
}
hk_on_exit() {
  local rc=$?
  set +x
  trap - EXIT ERR
  set +e +u
  hk_flush_config
  HK_PHASE=exit
  hk_log INFO "运行结束：rc=$rc sections=$HK_SECTIONS archived=$HK_SUCCEEDED failed=$HK_FAILED svn_committed=$HK_COMMITTED elapsed=$((SECONDS-HK_START_SECONDS))s cwd=$PWD log=$HK_LOG_FILE" || true
  if [[ -n "$HK_TEMP_DIR" && -d "$HK_TEMP_DIR" ]]; then rm -rf -- "$HK_TEMP_DIR"; fi
  exit "$rc"
}
trap 'hk_on_error "$?" "$LINENO" "${BASH_SOURCE[0]}" "${FUNCNAME[*]:-main}"' ERR
trap hk_on_exit EXIT
trap 'hk_log ERROR "收到中断信号 INT"; exit 130' INT
trap 'hk_log ERROR "收到终止信号 TERM"; exit 143' TERM

# 实时分开记录 stdout/stderr，关闭并等待日志读取器，保留原始命令退出码。
hk_run() {
  local label=$1 out_fd err_fd out_pid err_pid rc start=$SECONDS reader_rc=0 origin="${BASH_SOURCE[1]}:${BASH_LINENO[0]}"
  shift
  hk_log INFO "命令开始：$label program=$1 cwd=$PWD source=$origin（不记录原始参数）" || return 74
  if ! exec {out_fd}> >(hk_stream CMD-OUT "$label"); then return 74; fi
  out_pid=$!
  if ! exec {err_fd}> >(hk_stream CMD-ERR "$label"); then
    exec {out_fd}>&-
    wait "$out_pid" || true
    return 74
  fi
  err_pid=$!
  if "$@" >&"$out_fd" 2>&"$err_fd"; then rc=0; else rc=$?; fi
  exec {out_fd}>&-
  exec {err_fd}>&-
  if ! wait "$out_pid"; then reader_rc=74; fi
  if ! wait "$err_pid"; then reader_rc=74; fi
  if (( rc == 0 && reader_rc != 0 )); then rc=$reader_rc; fi
  if (( rc == 0 )); then
    hk_log INFO "命令完成：$label rc=$rc elapsed=$((SECONDS-start))s cwd=$PWD"
  else
    hk_log ERROR "命令失败：$label rc=$rc elapsed=$((SECONDS-start))s cwd=$PWD"
  fi
  return "$rc"
}

hk_log INFO "诊断脚本启动：source=$HK_SCRIPT_SOURCE startup_cwd=$HK_START_CWD script_dir=$HK_SCRIPT_DIR"
hk_log INFO "BASH=$BASH BASH_VERSION=$BASH_VERSION OSTYPE=$OSTYPE MSYSTEM=${MSYSTEM:-unset} stdin_tty=$([[ -t 0 ]] && printf yes || printf no) check_only=$HK_CHECK_ONLY"
hk_log INFO "PATH（加载配置前）=$PATH"
configuration_file="$HK_SCRIPT_DIR/config.sh"
repository_file="$HK_SCRIPT_DIR/repository.ini"
HK_PHASE=config
hk_log INFO "准备加载配置：$configuration_file readable=$([[ -r "$configuration_file" ]] && printf yes || printf no)"
if [[ ! -r "$configuration_file" ]]; then hk_log ERROR '配置文件不存在或不可读。'; exit 2; fi
hk_probe_encoding "$configuration_file" config.sh || exit 2
HK_CONFIG_PENDING=1
HK_CONFIG_ACTIVE=1
# 在当前 shell 加载变量；先缓冲配置输出，变量可用后统一脱敏，不打印源文件。
source "$configuration_file" > "$HK_TEMP_DIR/config.out" 2> "$HK_TEMP_DIR/config.err"
HK_SOURCE_RC=$?
set +x
HK_CONFIG_ACTIVE=0
# 配置可能修改 shell 选项，恢复诊断脚本的显式错误处理方式。
set +e +u
set -E -o pipefail
hk_flush_config
hk_log INFO "配置加载结束：rc=$HK_SOURCE_RC error_count=$HK_CONFIG_ERRORS cwd_after_source=$PWD"
if (( HK_SOURCE_RC != 0 || HK_CONFIG_ERRORS != 0 )); then hk_log ERROR '配置加载期间发生错误，未执行同步。'; exit 2; fi
cd -- "$HK_SCRIPT_DIR" || exit 2
hk_log INFO "配置加载后恢复脚本目录：$PWD"
hk_log INFO "PATH（加载配置后）=$PATH"

HK_PHASE=preflight
for key in svn_dir svn_username svn_password svn_timeout; do
  if [[ -z "${!key-}" ]]; then hk_log ERROR "缺少配置项：$key（不输出其值）"; exit 2; fi
done
if [[ ! "$svn_timeout" =~ ^[1-9][0-9]*$ ]]; then hk_log ERROR 'svn_timeout 必须为正整数。'; exit 2; fi
# 将配置中的 Windows 目录转为当前 Git Bash 路径；相对路径以脚本目录为准。
if [[ "$svn_dir" =~ ^[A-Za-z]:[\\/] ]] && command -v cygpath >/dev/null 2>&1; then
  svn_dir=$(cygpath -u -- "$svn_dir") || exit 2
fi
if [[ "$svn_dir" != /* ]]; then svn_dir="$HK_SCRIPT_DIR/$svn_dir"; fi
if [[ ! -d "$svn_dir" ]]; then hk_log ERROR "SVN 目录不存在：$svn_dir"; exit 2; fi
svn_dir=$(cd -- "$svn_dir" && pwd -P) || exit 2
hk_log INFO "SVN 配置检查：directory=$svn_dir timeout=${svn_timeout}s 用户名和口令均已配置"
hk_log INFO "仓库配置位置：$repository_file"
if [[ ! -r "$repository_file" ]]; then hk_log ERROR "仓库配置不存在或不可读：$repository_file"; exit 2; fi
hk_probe_encoding "$repository_file" repository.ini
for tool in git zip svn cp rm mkdir; do
  if ! command -v "$tool" >/dev/null 2>&1; then hk_log ERROR "命令未找到：$tool；请检查配置是否覆盖 PATH。"; exit 127; fi
  hk_log INFO "工具定位：$tool -> $(command -v "$tool")"
done
hk_run 'git version' git --version || exit 2
hk_run 'svn version' svn --version --quiet || exit 2
hk_run 'zip version' zip -v || exit 2
if (( HK_CHECK_ONLY )); then
  hk_log INFO '只读配置/工具检查完成；未执行 clone、fetch、merge、push、zip、SVN commit 或清理。config.sh 本身作为配置代码已加载。'
  exit 0
fi
printf -v current_time '%(%Y-%m-%d %H:%M:%S)T' -1

clone_repository() (
  local branch=$1 url=$2 target=$3 mergebranch=$4
  hk_run 'git clone' git clone --single-branch -b "$branch" -- "$url" "$target" || return 1
  cd -- "$target" || { hk_log ERROR "克隆后无法进入目录：$target"; return 1; }
  hk_log INFO "进入克隆目录：$PWD"
  if [[ -n "$mergebranch" ]]; then
    hk_run 'git fetch merge branch' git fetch origin "$mergebranch:refs/remotes/origin/$mergebranch" || return 1
    hk_run 'git merge' git merge -s recursive -X theirs "origin/$mergebranch" --no-edit -q || return 1
  else
    hk_log WARN '未配置 mergebranch，跳过 fetch/merge。'
  fi
  hk_run 'git push' git push origin "$branch" || return 1
  hk_run 'git HEAD' git rev-parse --short HEAD || return 1
)
compress_repository() {
  local directory=$1
  hk_run 'zip repository' zip -q -r -9 "$directory.zip" "$directory"
}
hk_remove_clone() {
  local target=$1 canonical
  canonical=$(cd -- "$target" && pwd -P) || return 1
  if [[ "$canonical" != "$HK_SCRIPT_DIR/"* || "$canonical" == "$HK_SCRIPT_DIR" || -L "$target" ]]; then
    hk_log ERROR "拒绝删除非脚本目录下的克隆目录：$target canonical=$canonical"
    return 1
  fi
  hk_run 'remove cloned directory' rm -rf -- "$target"
}
commit_to_svn() (
  local archive=$1 filename destination
  filename=${archive##*/}
  destination="$svn_dir/$filename"
  hk_log INFO "准备复制归档：source=$archive destination=$destination"
  hk_run 'copy archive to SVN' cp -f -- "$archive" "$destination" || return 1
  cd -- "$svn_dir" || { hk_log ERROR "无法进入 SVN 目录：$svn_dir"; return 1; }
  hk_log INFO "进入 SVN 目录：$PWD commit_file=$filename"
  hk_run 'svn commit' svn commit --non-interactive --username "$svn_username" --password "$svn_password" \
    -m "源码同步 - $current_time" --force-log --config-option "servers:global:http-timeout=$svn_timeout" "$filename" || return 1
)

hk_process_repository() {
  [[ -n "$HK_SECTION" ]] || return 0
  HK_PHASE="repository:$HK_SECTION"
  hk_log INFO "读取完整配置：section=$HK_SECTION branch=${branch:-缺失} mergebranch=${mergebranch:-未配置} directory=${directory:-缺失} url=$([[ -n "$url" ]] && printf 已配置 || printf 缺失)"
  if [[ -z "$branch" || -z "$url" || -z "$directory" ]]; then
    hk_log ERROR '仓库配置不完整，跳过该仓库。'
    HK_FAILED=$((HK_FAILED+1)); return 0
  fi
  # 只处理脚本目录内新建的相对目录，避免后续 rm -rf 越界。
  case "/$directory/" in *'/../'*|*'/./'*|*'//'*) hk_log ERROR "不允许的克隆目录：$directory"; HK_FAILED=$((HK_FAILED+1)); return 0 ;; esac
  if [[ "$directory" == /* || "$directory" =~ ^[A-Za-z]: || "$directory" == *\\* ]]; then
    hk_log ERROR 'directory 必须是脚本目录下的相对目录（使用正斜杠）。'; HK_FAILED=$((HK_FAILED+1)); return 0
  fi
  local target="$HK_SCRIPT_DIR/$directory" archive="$HK_SCRIPT_DIR/$directory.zip"
  if [[ -e "$target" || -L "$target" || -e "$archive" ]]; then
    hk_log ERROR "目标目录或归档已经存在，保留现场并跳过：$directory"; HK_FAILED=$((HK_FAILED+1)); return 0
  fi
  if ! clone_repository "$branch" "$url" "$target" "$mergebranch"; then
    hk_log ERROR "Git 阶段失败；保留克隆目录供检查：$target"; HK_FAILED=$((HK_FAILED+1)); return 0
  fi
  if ! compress_repository "$directory"; then
    hk_log ERROR "压缩失败；保留目录及可能的归档：$target"; HK_FAILED=$((HK_FAILED+1)); return 0
  fi
  HK_ARCHIVES+=("$archive")
  HK_SUCCEEDED=$((HK_SUCCEEDED+1))
  if ! hk_remove_clone "$target"; then HK_FAILED=$((HK_FAILED+1)); fi
  hk_log INFO "仓库归档完成：$archive"
}
hk_trim() {
  local text=$1
  text="${text#"${text%%[!$' \t']*}"}"
  text="${text%"${text##*[!$' \t']}"}"
  printf '%s' "$text"
}

HK_PHASE=repository-config
HK_SECTION=''
branch='' mergebranch='' url='' directory=''
HK_LINE=0
while IFS= read -r line || [[ -n "$line" ]]; do
  HK_LINE=$((HK_LINE+1))
  line=${line%$'\r'}
  if (( HK_LINE == 1 )); then line=${line#$'\xEF\xBB\xBF'}; fi
  line=$(hk_trim "$line")
  [[ -n "$line" && "$line" != \#* && "$line" != \;* ]] || continue
  if [[ "$line" == \[*\] ]]; then
    hk_process_repository
    branch='' mergebranch='' url='' directory=''
    HK_SECTION=''
    if [[ "$line" == \[repository*\] ]]; then
      HK_SECTION=$line
      HK_SECTIONS=$((HK_SECTIONS+1))
      hk_log INFO "开始读取仓库节：$line line=$HK_LINE"
    else
      hk_log WARN "忽略非 repository 节：line=$HK_LINE"
    fi
    continue
  fi
  [[ -n "$HK_SECTION" ]] || continue
  if [[ "$line" != *=* ]]; then hk_log WARN "忽略无等号的配置行：line=$HK_LINE"; continue; fi
  key=$(hk_trim "${line%%=*}")
  value=$(hk_trim "${line#*=}")
  if [[ "$value" == \"*\" || "$value" == \'*\' ]]; then value=${value:1:${#value}-2}; fi
  case "$key" in branch) branch=$value ;; mergebranch) mergebranch=$value ;; url) url=$value ;; directory) directory=$value ;; *) hk_log WARN "忽略未知配置项：key=$key line=$HK_LINE" ;; esac
done < "$repository_file"
hk_process_repository
if (( HK_SECTIONS == 0 )); then hk_log ERROR 'repository.ini 中未找到 repository 节。'; HK_FAILED=$((HK_FAILED+1)); fi

for zip_file in "${HK_ARCHIVES[@]}"; do
  HK_PHASE="svn:${zip_file##*/}"
  if commit_to_svn "$zip_file"; then
    HK_COMMITTED=$((HK_COMMITTED+1))
    if ! hk_run 'remove committed archive' rm -f -- "$zip_file"; then HK_FAILED=$((HK_FAILED+1)); fi
  else
    HK_FAILED=$((HK_FAILED+1))
    hk_log ERROR "SVN 阶段失败；保留本地归档：$zip_file"
  fi
done
HK_PHASE=summary
hk_log INFO "同步汇总：sections=$HK_SECTIONS archived=$HK_SUCCEEDED svn_committed=$HK_COMMITTED failed=$HK_FAILED"
if (( HK_FAILED > 0 )); then exit 1; fi
exit 0
