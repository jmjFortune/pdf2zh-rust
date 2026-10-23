# 进度记录

## 2026-10-22 Codex

- 用户选择“整个重写”，按独立 Rust 替代上游 pdf2zh-next 与 BabelDOC 处理，未把请求解释成只包一层 Rust。
- 读取当前仓库 AGENTS.md 和既有交接，确认当前 translator 前后端不在本次重写范围。
- 检查 NAS 已安装源文件和独立导入内存测试结果，查询 Rust PDF 绑定与 BabelDOC primary sources。
- 使用 brainstorming 与 planning-with-files 组织方案；先形成可审阅设计，再按技能要求取得设计确认。
- 向用户异步询问是否允许调用成熟 C/C++ 原生库；继续进行不依赖此答案的上游能力审计。
- 当前仅新增 docs/rust-rewrite 下的文档；无代码、依赖安装、NAS 容器或代理变更。

## 2026-10-22 Codex（原生库路线确认与设计）

- 用户了解两条路线的差异后明确选择 Rust＋成熟原生库；后续不再把纯 Rust 作为约束。
- 核对 PyMuPDF→MuPDF 和 ONNX Runtime 的官方原生接口/Rust 绑定；明确 PDFMiner/BabelDOC 等 Python 逻辑不能自动通过 C 库接口取代。
- 完成 design.md：定义无 Python 的 Rust 应用、原生库边界、按任务退出的 worker、PDF 处理链、阶段交付和质量/内存测量。
- 设计自审检查范围、用户最新选择、待机/峰值内存口径、上游覆盖、未实现声明及独立 NAS 工作目录；没有用任意内存估算代替实测。
- 实施尚待详细设计审阅；当前不安装 Rust 工具链、不下载依赖到 Mac、不改原容器。

## 2026-10-22 Codex（用户批准实施）

- 用户回复“OK”，详细设计已批准，开始 NAS 独立实现。
- 核实目标目录不存在；父目录未发现 AGENTS.md。NAS 有 cc/make/git/curl，尚无 PATH 中的 Rust/Cargo，已有原版镜像及 Debian 镜像可复用。
- 补充 implementation-plan.md；缺少 writing-plans 技能，已搜索现有技能和插件后采用文档计划替代。
- 现有 pdf2zh-next 容器仍运行；不修改本项目业务代码或原服务。

## 2026-10-22 Local development

- User changed the development location to this Mac. Workspace: `/Users/shiyun/Desktop/pdf2zh-rust`, branch `agent/codex/rust-engine`. This later instruction overrides the earlier NAS-only build location.
- Rust 1.96.0 and Clang already installed; reused existing cached font and layout model read-only. No Docker image downloaded on Mac.
- Native MuPDF 0.8.0 build succeeded. PDF roundtrip test passed: searchable Chinese, original body removed, protected formula/vector pixels unchanged.
- ONNX model integration in progress. This is not a complete engine yet.

## 2026-10-22 Native pipeline and Web service

- Rust MuPDF font/glyph extraction, selective text redaction, searchable Chinese output, and pixel preservation of protected formula/vector fixture passed.
- Native ONNX Runtime 1.31.0 loaded through ort 2.0.0-rc.15; real 72 MiB layout model produced labeled detections. Reused existing native library, model and font; no Python execution in the application.
- Real Google translation completed for the synthetic public test paragraph; mono and vector-preserving side-by-side PDFs rendered and visually inspected successfully.
- Added CLI page selection, layout, translation/cache, font fitting with explicit overflow error, atomic output validation, Web upload/history/cancel/SSE/download and process-per-task supervision.
- Four current Rust tests passed before Web smoke testing. Web tests and memory measurements in progress.
- Remaining significant scope: complex inline math anchored reflow, cross-page paragraph semantics, table text translation, OCR, remaining upstream providers/config compatibility, production packaging and NAS deployment. Development preview explicitly lists incomplete capabilities.

## 2026-10-23 本机开发版验证

- 本机独立项目完成 Rust 服务、按任务退出的翻译 worker、按阶段退出的原生模型 analyzer；应用不调用 Python。模型进程结束后才进入远程翻译，避免等待接口期间驻留约 1.3 GiB 的推理分配。
- 原生 PDF 处理补齐整段公式标记重排、原公式字形保留、ActualText 精确中文 Unicode、阅读顺序、旋转/裁剪页面、链接/书签、压缩保存。字体子集化尚未实现。
- 11 项测试通过；合成 PDF 与公开论文选页通过真实 Google 检查。OpenAI 路径使用明确标注的本地模拟协议测试，其他服务商不冒充实际验证；Bing 实测 401 已记录。
- 独立 bundle 从 /private/tmp 启动，网页上传、并排译文、PNG 预览和浏览器下载均通过；入口 http://127.0.0.1:7861/。附带 Run.command，未设置自动启动。
- 内存分阶段实测、截图路径及边界见 verification.md；模型瞬时峰值仍约 1.3 GiB，尚无 NAS 同环境对比。
- 未完成：完整上游参数/服务商兼容、跨页段落、完整表格、OCR、多语言塑形、字体子集化及生产/NAS 打包。当前为开发版，不标记整个重写完成。

## 2026-10-23 NAS 部署验证

- 用户授权部署后，在 NAS 拉取固定 cloud digest 并启动独立非 root 容器，私网 7861、独立卷、健康检查与 restart policy 正常。
- 首个真实翻译暴露既有默认代理出口的 Google TLS 超时。对照证明另一既有出口可用；只加翻译 API 域名规则并热重载，保留其他 selector 与代理进程，随后真实 Google 任务通过。
- 单页合成 PDF 完成于 29.59 秒；原生布局、中文公式、并排 PDF、PNG 预览下载和浏览器展示均验证。重启仅新 Rust 容器后，历史和输出哈希保持。
- NAS 100 ms cgroup 采样：working set 峰值 432.45 MiB，结束后 1.94 MiB；后续 Docker stats 为 1.934 MiB，重启后 1.359 MiB。与早先 Mac RSS 分开记录，不宣称大文档或上游同输入对照已完成。
- 一个文本层精确性问题已记录：视觉页面正确，原生提取末尾多出“变。”碎片。未为本次部署修改排版代码；后续应以这个样本补精确回归。完整重写仍在进行。

## 2026-10-23 HTTP 400 / misleading late progress investigation
- NAS failed task reports translate page 1/47, zero completed blocks; one shared cache row belongs to the earlier smoke task. User confirms API key was supplied. Input PDF retained; terminal job.json and provider response body were deleted by old behavior, so the exact rejection reason is not yet known.
- Fix in progress: translate first selected page before analyzing remainder, label analysis as its own phase, bounded/redacted provider errors and selective retry, retry using retained input and explicitly supplied settings. Verify with rejecting mock provider and real worker lifecycle, then cloud-build/deploy while idle. Real OpenCode validation requires the user's key to be re-entered in the UI; do not request keys in chat.

- Completed local/cloud checks and NAS upgrade to verified source 80d499b/digest d916e2f…; original records/input hashes retained. NAS mock-error/retry regression passed (56.48 s), browser retry UI verified. Real OpenCode request and original 47-page translation await re-entered user credentials; do not claim them completed.

## 2026-10-23 font recovery in progress
- Real task failed at page 32/47, 510 translated blocks, missing primary-font U+2023. NAS cache snapshot contains 512 rows and two translated values with U+2023; backup saved before changes.
- Scoped repair: select a validated mature fallback font only for paragraphs unsupported by the primary font; same bytes used for measurement and output, preserve exact Unicode. Add cache-only recovery path to prevent re-requesting cached content. Verify missing-glyph rich-formula regression, then cloud build/deploy. Recover prior pages using a private local copy of the user's NAS input/cache and existing local model; do not transmit the PDF to any new service or put data in Git.

## 2026-10-23 — translator UI integration
Copied the existing frontend without changing original project code. Added a separate Rust adapter rather than redirecting it to a Python backend. Local static build and browser upload/refresh/library/reader/download checked. New native integration test validates private saved keys, public metadata, SSE reconnect sequencing, three exports and cache reuse after retained-input retry. Existing fourteen native tests also passed; final suite rerun and cloud/NAS publication are in progress. Preserve original user task and its 512-row cache.

Final UI deployment verified: cloud run 39951931277 succeeded; NAS source de39b82 is healthy with the same named volume and all eight retained task records. Original cache-only page-32 regression passed 13 blocks, then stopped on missing cache as designed. User confirmed manual Library clearing; do not restore hidden records. See verification.md for the in-app PDF viewer limitation and idle memory samples.
