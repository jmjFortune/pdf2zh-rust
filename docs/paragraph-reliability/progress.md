# 段落可靠性修复进度

2026-10-29：用户已批准先修连字符和安全降级。完成代码路径核查、预存改动快照、实施边界与验收定义；尚未修改业务源码。

新增回归在旧实现上复现两个连字符测试失败，以及普通排版溢出导致整份失败。实施明确的 InvalidTranslation / Overflow 类型，仅在原文删除前降级；其他错误沿用原失败路径。跨页尝试批量与逐段回退分开，避免后段失败丢掉前段结果。

首轮聚焦测试通过两个连字符回归、三项段落真实 PDF 检查和两项 UI 检查。失败原段/行内代码的渲染区域与输入像素一致，成功段落缓存复用，告警经任务快照及 SSE 并在重启后保留。补充带公式溢出和跨页失败回归，完整测试正在运行。

完整工作区 39 项 release 测试全部通过；将候选提交导出到忽略目录、剔除预存 thinking 修改，再运行完整 release 测试：37 项全部通过（21 单元、3 跨页、4 段落可靠性、6 PDF、1 生命周期、2 UI）。两次均无失败；格式和 diff 检查通过。独立快照用既有字体、模型和原生运行库，所有 HTTP 验收为本地 mock，无付费接口调用。

验证日志：artifacts/paragraph-reliability-tests.log、paragraph-reliability-clean-tests.log。用旧 thinking diff 重建当前文件后 cmp 完全一致，当前未暂存 diff 仍是原有 170 插入/7 删除；GAP 文档 SHA256 保持。新增代码测试按独立候选暂存，文档更新后本地提交；NAS 与 GitHub 未改。

用户随后要求推送 GitHub 与 NAS。源码 ca8e414111cb7e7c0e56d619bb0290b092d4d17b 推送后，Actions 40889155664 / job 123500896798 通过 37 项 Linux 测试、前端生产构建和最终容器检查并发布 digest a0dec2b1e2f43dd29d03c94ce0ef4001cf8aec67bc0fa5890f28825b75cfc54b。

NAS 仅拉取该固定 digest；隔离 fixture / loopback 验收通过连字符、原段保留、完成告警、有效缓存复用、无效译文不缓存、全部失败/401/取消拒绝，失败区域 PNG 像素一致且渲染已查看。相同镜像跨页复验通过 1 次联合请求、正确原页回填、缓存重跑零请求。测试不挂正式卷、不使用付费接口。

确认空闲后备份私有配置与 SQLite，升级独立 Rust 容器；最终只读检查 verified、源码 ca8e414、healthy、无活动任务，保留原卷、11 条任务、22 个 PDF 哈希、私有设置以及其他容器 ID/启动时间。报告为 artifacts/reliability-{cloud-result,nas-smoke-result,nas-cross-page-smoke-result,nas-rollout-result,nas-final}.json；NAS 私有备份目录 diagnostic-reliability-20261029-141247。Mac 未构建/下载镜像，旧产物未重译；原 Python 容器、代理保持。
