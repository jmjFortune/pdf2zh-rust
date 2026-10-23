## 2026-10-23 Codex

- 做了什么：在独立 Mac 项目实现并验证 Rust＋原生库 PDF 翻译开发版，原生模型按阶段退出，网页支持上传、进度、取消、历史、预览和下载。
- 改了哪些文件：src/、tests/、web/index.html、Cargo 配置、资源清单、scripts/Run.command 与项目文档；详见 Git 提交。原 translator 仅更新交接文档。
- 验证：11 项测试通过、release 构建通过；bundle 从 /private/tmp 运行，浏览器完成真实 Google 翻译、PNG 预览及 PDF 下载。指标和测试边界见 verification.md。
- 没做完 / 已知问题：完整上游替代未完成；OCR、表格、跨页语义、完整参数/服务商、多语言塑形、字体子集化和 NAS 对比仍待实现；Bing 实测 401。
- 下一个接手的人先看：README.md、task_plan.md、verification.md、design.md。入口 http://127.0.0.1:7861/；独立 bundle 位于 dist/pdf2zh-rust/，数据和构建产物均不入库。

## 2026-10-23 Codex（用户授权推送）

- 做了什么：按用户“推送吧”建立私有远端 https://github.com/jmjFortune/pdf2zh-rust ，将已验证的源码检查点准备推送至 agent/codex/rust-engine；不直接提交 main。
- 改了哪些文件：本交接记录；origin 配置指向独立 Rust 仓库。模型、字体、原生运行库、构建产物、PDF、用户数据和临时截图均不入库。
- 验证：源码未变，沿用前次 11 项测试与 release 验证；推送前工作区和 diff 检查通过，受跟踪文件凭证模式扫描未发现匹配。推送结果以远端分支 SHA 与本地 HEAD 比较为准。
- 没做完 / 已知问题：本次仅推送源码；Docker/CI/镜像仓库和 NAS 部署尚未实施，开发版既有边界保持。
- 下一个接手的人先看：README.md、verification.md；后续构建部署建议采用 Compose、Buildx 缓存和 GitHub Actions 镜像流水线。

## 2026-10-23 Codex（公开仓库与 Docker 构建）

- 做了什么：用户明确选择 pdf2zh-rust 并授权公开后，已确认仓库 PUBLIC；接入标准 Ubuntu 24.04/amd64 的 Docker 构建、Cargo Chef/Buildx 缓存和通过测试后发布 GHCR 的流程。
- 改了哪些文件：Dockerfile、.dockerignore、compose.yaml、.github/workflows/docker.yml、scripts/fetch-container-assets.sh、assets/manifest.json、README 与 Docker 计划/说明及本记录。镜像内携带固定哈希的原生推理库、模型、字体及许可证。
- 验证：本机 actionlint 1.7.12、shell 语法、Compose 配置、JSON 和 git diff 检查均通过，当前 11 项 Rust 测试通过。云端构建证据随后写入 verification.md；尚未把编写配置视作构建通过。
- 云端结果：源码提交 8c5dbb4 已推送；Actions run 39885551001 成功，job 用时 9 分 34 秒。Linux 11 项测试通过，最终镜像的资源哈希、真实推理、HTTP 服务和非 root 检查通过，已发布 dev 和 sha-8c5dbb4b2d68 标签。无凭证 GHCR manifest 请求返回 200，digest 与发布日志一致。
- 没做完 / 已知问题：NAS 未部署；上游完整功能覆盖仍待完成，当前为开发版。GitHub 个人包元数据 API 需要额外 read:packages 权限，当前未申请或扩权；匿名 registry 接口已独立验证可拉取，无需因此修改凭证。
- 下一个接手的人先看：docker-build-plan.md、docker.md 和 Actions 的 Docker image 日志；构建自动触发范围限定源码和构建相关路径，无定时任务、付费 runner 或 NAS 自动部署。

## 2026-10-23 Codex（NAS 开发版已部署）

- 做了什么：按用户“部署”在 NAS 建立独立 Rust 容器、持久化卷和私网 7861 入口，固定已验证镜像 digest；仅对 Google 翻译域名增加经探测验证的代理规则，备份配置并保留其他出口选择，代理容器未重启。
- 改了哪些文件：NAS 私有部署 Compose/env 与单域名代理配置；本仓库仅更新 README、Docker 计划/说明、verification、进度与本交接，无应用代码或 API 契约修改。地址、完整配置、测试样本和截图均在忽略目录，不入库。
- 验证：容器 healthy、UID 10001、资源哈希正常；NAS 与客户端 HTTP 200。真实 Google 单页任务 29.59 秒完成，中文/公式、PDF 下载、PNG 预览和浏览器展示通过；空闲重启后历史与输出哈希保持。NAS cgroup 采样峰值 working set 432.45 MiB，结束后约 1.94 MiB；非完整性能基准。
- 没做完 / 已知问题：仍是开发版，完整上游覆盖和同输入对照未完成；该样本原生文本提取还存在尾部重复碎片，已在 verification 记录，不把视觉正确等同于全文提取完全正确。首个失败的网络测试任务保留在历史中；第二次已成功。没有操作原版容器生命周期。
- 下一个接手的人先看：verification.md 的 NAS 记录、docker.md；本机忽略的 artifacts/nas-deployment-record.json 保存私有入口和 NAS 部署目录，下一项代码工作可先补文本层精确回归。


## 2026-10-23 Codex（HTTP 400 与误导进度修复）
- 做了什么：确认真实 47 页任务失败于第一页首段；旧进度把全篇版面分析显示得像翻译接近完成。改为第一页分析/翻译后再分析余页，进度明确区分阶段和已完成正文段数；补应用/会话标识、受限脱敏错误原因、永久 HTTP 错误不盲重试，以及复用保留原文件、显式提供当前设置的新任务重试接口/按钮。
- 改了哪些文件：src/engine.rs、src/translate.rs、src/service.rs、web/index.html、tests/service_lifecycle.rs 及 API/验证/进度文档。当前 translator 业务源码未改。
- 验证：cargo fmt --check、cargo test --locked -j 2（7 单元 + 4 PDF + 1 进程生命周期，共 12 项）、release build、浏览器脚本语法和 git diff --check 通过。将 engine/translate 临时还原 HEAD 后新生命周期测试确实失败，随后恢复修复并跑完整套通过。
- 没做完 / 已知问题：原 HTTP 错误体被旧版丢弃，Key 也按原设计清除；缺少 OpenCode session 是有文档与官方仓库同类 400 报告支持的候选原因，未用用户 Key 实际确认。云端构建及 NAS 升级待下续验证。重试是利用段落缓存重新生成，尚无部分 PDF/逐页检查点，不声称可恢复此前未生成的译文。
- 下一个接手的人先看：docs/verification.md 和 docs/api.md；保持旧失败记录/输入，不从无关应用提取 Key，不把 Key 发聊天，不在 Mac 拉 Docker。NAS 升级只在无活跃任务时操作 Rust 容器并保留原卷。

- 后续云端/NAS 验证：Actions 39916128410 全部成功，Linux 同样 12 项测试和最终容器检查通过；镜像 digest d916e2f53f107e47e3b8cdb9b58e3d2ff1fc36355d2bb69699a0a644654aaa48 已在 NAS 拉取、校验 revision 80d499b 并在无活跃任务时升级，容器 healthy。原三条任务记录逐字段不变、所有原输入 SHA256 不变、数据卷名称保持。
- NAS 原生 worker/HTTP 回归：私有局域网 mock 返回单次 HTTP 400，保留结构化原因且 Key 被脱敏；复用保留输入重试产生新的 completed 任务，PDF 下载通过，旧失败记录仍在，两个私有 job.json 均被删除，合计 56.48 秒。该验证服务明确是本地 mock，不能写成真实 OpenCode 联调成功。
- 网页已核对原用户任务：显示 translate 1/47、0 段及重试入口；在保留的浏览器页预填用户截图里的服务地址和模型，API Key 留空。用户须在页面重填 Key 后点重试，才能确认 OpenCode 的真实拒绝是否消除并开始原文翻译。尚未自动重跑用户整份 PDF。


## 2026-10-23 Codex（运行中任务“卡住”只读排查）
- 做了什么：用户截图停在 translate 14/47、235 段；SSH 核查同一个任务实际已到 244 段，随后 20 秒采样从 15/47、249 段推进到 17/47、253 段，status running、无 error。确认当前任务使用 OpenCode / deepseek-v4.1-flash，而非截图左侧新建表单的默认 Google；只输出白名单设置及 Key 是否存在，不输出凭证。
- 改了哪些文件：仅本交接与验证说明；没有代码、配置或镜像修改。
- 验证：worker 存活并在 epoll 等待 HTTP，Docker working set 约 77.63 MiB、CPU 0.22%；模型 analyzer 已退出，OpenCode 连接有收发数据。现有逐段串行调用、按页更新进度条会产生停顿感，已有连续进展证明无需终止任务。SSE 连通性结果见后续补充。
- 没做完 / 已知问题：全文尚在运行，未声称 PDF 已完成；未确认用户 Edge 的事件连接是否曾暂时断开。未取消、重启、重试、切换供应商或改代理，避免中断已有译文。需要时刷新页面后从历史点查看即可重新订阅。
- 下一个接手的人先看：同一任务的 GET /api/jobs/{id} 最新状态；使用原有缓存和原输入，不因页面暂时不动就重启 worker。私有任务 ID 不写入公共文档。
- 后续验证：NAS SSE 请求 HTTP 200、Content-Type text/event-stream，并收到同一任务 running、17/47、257 段的 job 事件，证明服务器事件接口也正常；仍不把它等同于用户 Edge 当时的订阅状态。

## 2026-10-23 Codex（U+2023 字体缺字修复与离线恢复）
- 做了什么：确认 32/47 页、510 段后的真实失败为主字体无 U+2023；备份 NAS 原始缓存（512 条）。新增按段落实际文字选择成熟备用字体，排版测量和写入同字体，公式占位符不当作可见字体字符；不替换或删除原符号。新增仅缓存恢复环境开关，缺缓存时禁止联网。
- 改了哪些文件：src/fonts.rs、engine/lib/translate、资源 manifest 与 NOTICE、真实进程回归，以及 Docker repair target / workflow 和文档；原 translator 业务代码未改。
- 验证：回归还原旧 engine 后复现原 U+2023 错误；新测试覆盖中文/符号/公式、原字符可搜索及 cache-only 无额外请求。完整本机套件与 release 结果随后记录。离线发布版已从原缓存恢复前31页、498段，文件仍47页，32–47页是英文；第1、31页已渲染核对。私有 PDF/缓存/截图不入库。
- 没做完 / 已知问题：原文尚未全文完成，NAS 新镜像待构建/部署；旧 Key 已清除，真实后续翻译待重填。调试版实际恢复在第23页遇到 MuPDF ActualText 断言，发布版已完成相同恢复；该调试原生库问题单独保留，不声称字体修复涵盖它。当前仍为开发版。
- 下一个接手的人先看：docs/verification.md、docs/docker.md；保留 NAS 原缓存/失败记录，不将部分恢复版标记全文译完。修复镜像复用已验证模型/系统层，只增字体和二进制，所有编译仍在 GitHub 云端。

## 2026-10-23 Codex（translator 前端复用）
- 做了什么：按用户要求复制 translator 的 Next.js 界面至独立 Rust 项目；添加 /api/ui 适配、Rust 静态文件服务、设置/术语库、全部任务历史、保留原文件重试、真实阶段 SSE、多格式一次翻译导出。保留旧 API 与字体修复。
- 改了哪些文件：web/、src/service.rs 与 src/service/ui.rs、src/engine.rs、tests/ui_adapter.rs、Docker/CI 配置、docs/api.md 和集成计划。原项目代码未修改；D-038 记录跨项目复用。
- 验证：本机生产静态构建通过（首次 JS 101 KB）；既有 14 项测试与新增真实进程适配测试通过；浏览器完成上传、三格式选择、任务刷新恢复、文库、阅读器和实际下载。
- 没做完 / 已知问题：NAS 发布待本记录后的云端构建和部署验证；旧任务未记录的服务设置不会猜测。Zotero/OCR/结构化阅读暂未实现；原界面桌面布局在很窄的侧栏中需要展开浏览器。
- 下一个接手的人先看：frontend-integration-plan.md、api.md，以及原有字体恢复记录。仅保留前 31 页的已恢复用户 PDF，不声称 47 页已全部翻译。

## 2026-10-23 Codex（NAS 前端替换验收完成）
- 做了什么：最终 de39b82 镜像已部署，NAS 健康，原数据卷与全部输入哈希/任务保留。实际上传、刷新恢复、三种 PDF 导出和字体故障点验证通过；空载 Docker 工作集约 2–7 MiB。
- 改了哪些文件：前端/适配代码在 1aa9e15，分析阶段无百分比修正在 de39b82；本条和 verification/progress 记录部署证据。原 translator 的源码没有改动。
- 没做完 / 已知问题：用户确认手动清空文库，保持隐藏、不自动恢复；完整 47 页原 PDF 尚未完成。内置浏览器不能显示独立控制 PDF，Edge 视觉预览受权限限制未验证。其他 Rust 功能边界见 api.md。
- 下一个接手的人先看：verification.md 最新验收、api.md、frontend-integration-plan.md。最终镜像 sha256:d0d850c12286617b0369639fb828b557b7e0063fbe78b65c803a4e125c454d79；NAS 原文件与缓存仍保留，OpenCode 密钥需用户在 Settings 主动保存。

## 2026-10-23 Codex（About 移除、服务补齐、Dual 命名）
- 做了什么：按用户要求移除 About 页面/入口/专属下载；Side by Side 可见文案改为 Dual，并同步导出优先级与文件名，Interleaved 保留。服务扩为 20 个配置入口，补 Azure 参数及模型/地址缺失的排队前检查。
- 改了哪些文件：web/src/、专属公开下载、src/service/ui.rs、profiles.json、ui_tests.rs、translate.rs 的可选空密钥处理、tests/ui_adapter.rs、Docker/CI 与接口/计划文档。
- 验证：本机 17 项测试和静态生产构建通过；新增本地 HTTP 协议矩阵检查 19 个非 Google 配置的路由/请求/独立密钥，Google 沿用先前实测。浏览器确认 20 个入口、无 About、旧 About 链接回退及 Azure 参数字段。
- 没做完 / 已知问题：云端发布、NAS 最终部署待本条后的验收；未调用用户付费账户逐家实测。原有 PDF、缓存、密钥、文库隐藏状态保持。
- 下一个接手的人先看：provider-ui-plan.md、api.md 最新说明。API 的 side_by_side/dual 旧标识不互换，产品名分别是 Dual/Interleaved。
