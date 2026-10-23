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
