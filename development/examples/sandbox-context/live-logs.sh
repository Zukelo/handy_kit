#!/usr/bin/env bash
# Synthetic stdout/stderr probe. No business files or repositories are accessed.
printf '实时日志测试开始；MSYSTEM=%s\n' "${MSYSTEM-}"
if shopt -q login_shell; then printf '登录 Shell=是\n'; else printf '登录 Shell=否\n'; fi
printf '这是一条标准错误日志\n' >&2
for step in 1 2 3; do
  printf '进度 %s/3\n' "$step"
  sleep 1 || exit 2
done
case "${1-0}" in
  23) printf '测试结束：按参数返回退出码 23\n'; exit 23 ;;
  *) printf '测试结束：退出码 0\n'; exit 0 ;;
esac
