#!/bin/sh
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
printf '%s\n' '本机服务：http://127.0.0.1:7861/' '保持此窗口打开；按 Ctrl+C 停止。'
exec ./pdf2zh-rust serve --listen 127.0.0.1:7861 --data-dir data
