#!/usr/bin/env bash
# Read-only environment probe; writes only its own report beside this script.
# Uses Bash builtins so a broken PATH does not prevent diagnostics.
set +x
umask 077
script=${BASH_SOURCE[0]}
case "$script" in
  */*) directory=${script%/*} ;;
  *) directory=. ;;
esac
report="$directory/handykit-context-$$-${RANDOM}.log"
set -C
if ! exec 3> "$report"; then
  printf '无法创建诊断报告：%s\n请将脚本放在可写目录后重试。\n' "$report" >&2
  exit 1
fi
set +C
printf '诊断报告：%s\n' "$report"
{
  printf 'HandyKit Git Bash context report\n'
  printf 'MSYSTEM=%s\nBASH=%s\nBASH_VERSION=%s\nHOME=%s\n' "${MSYSTEM-}" "$BASH" "$BASH_VERSION" "${HOME-}"
  printf 'Shell参数=%s\n' "$-"
  if shopt -q login_shell; then printf '登录Shell=是\n'; else printf '登录Shell=否\n'; fi
  printf '说明：此处是执行脚本的 Shell；交互标志可能与手动打开终端不同。\n'
  printf '工作目录='; builtin pwd -P
  printf '脚本=%s\n' "$script"
  printf '\n工具位置：\n'
  for tool in bash sh git svn; do
    printf '%s: ' "$tool"
    command -v "$tool" || printf '未找到\n'
  done
  printf '\n初始化文件可读性（不读取内容）：\n'
  for file in /etc/profile /etc/bash.bashrc "${HOME-}/.bash_profile" "${HOME-}/.bashrc"; do
    if [[ -r "$file" ]]; then printf '可读 %s\n' "$file"; else printf '不可读或不存在 %s\n' "$file"; fi
  done
  printf '\n指定路径可见性（不读取内容）：\n'
  for file in "$@"; do
    printf '路径=%s 存在=' "$file"
    if [[ -e "$file" ]]; then printf '是'; else printf '否'; fi
    printf ' 可读='
    if [[ -r "$file" ]]; then printf '是\n'; else printf '否\n'; fi
  done
  printf '\n报告完成\n'
} >&3 2>&3
exec 3>&-
exit 0
