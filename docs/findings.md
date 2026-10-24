# 核查记录（外部资料为研究数据）

## 现场基线

- NAS 当前容器 pdf2zh-next，镜像 awwaawwa/pdfmathtranslate-next:latest；已部署版本 pdf2zh-next 2.9.0、BabelDOC 0.6.2、Gradio 5.35.0。
- 原容器空闲 docker stats 约 334.4 MiB，Linux 进程 RSS 约 381 MiB；统计口径不同，不直接相减。
- 在容器中启动独立 Python 进程逐项测量 RSS：Python 8256 kB；import gradio 104772 kB；import pdf2zh_next.high_level 260996 kB；import pdf2zh_next.gui 316568 kB。独立测量不能相加，也不是各模块可直接削减的内存。
- main.py 顶层导入翻译模块；启动时执行 babeldoc.assets.assets.warmup()；包 __init__.py 也直接导出 high_level 的接口。
- high_level.py 直接调用 babeldoc.format.pdf.high_level.async_translate；完整重写必须覆盖真实 BabelDOC 流程。

## 已查 primary sources

- https://github.com/PDFMathTranslate-next/PDFMathTranslate-next ：上游将自己定义为调用 BabelDOC 的官方参考实现，具备 GUI、CLI、多服务商和 Docker。
- https://github.com/funstory-ai/BabelDOC ：真实 PDF 翻译和双语排版内核。
- https://docs.rs/pdfium-render/latest/pdfium_render/ ：Rust PDFium 绑定，原生 PDFium 需要另行提供，并非纯 Rust PDF 引擎。
- https://docs.rs/mupdf/latest/mupdf/ ：MuPDF 的 Rust 封装，有文字结构、绘制和 PDF 文档相关接口；最终选型仍需基于所需对象修改能力验证。
- https://pyo3.rs/main/ ：Rust 嵌入 Python 仍需 Python 解释器，不符合本次最终无 Python 运行时的范围。

## 研究边界

- 没有执行真实 PDF 翻译基准，不能据此宣称 Rust 比原版更快或排版等价。
- 用户先选择所有底层纯 Rust，询问差异后明确改选 Rust＋成熟原生库；允许复用 C/C++，最终不保留 Python。
- 低层 PDF 编辑 API 的存在不等于具备 BabelDOC 的语义解析、公式保护和译文重排算法。

## 原生复用进一步核查

- https://pymupdf.readthedocs.io/en/latest/about.html 的实现对照将 PyMuPDF 列为 Python/C、基于 MuPDF；也将 PDFMiner 列为 Python，不能泛称所有 Python 库都是 C 封装。
- https://mupdf.readthedocs.io/en/latest/reference/c/index.html 提供 MuPDF C 库接口；Rust 绑定可用于替代 Python 调用，但需要核实高级接口的覆盖。
- https://onnxruntime.ai/docs/get-started/with-c.html 提供原生推理入口；https://docs.rs/ort/latest/ort/ 明确 `ort` 是 ONNX Runtime Rust 绑定。
- https://docs.rs/mupdf/latest/mupdf/pdf/page/struct.PdfPage.html 为从官方目录点击确认的页面接口地址；存在插入字体、写入、文字及 PDF 对象相关能力，不据此宣称已验证完整排版。
- 已安装源文件静态计数：pdf2zh_next 35 个 .py / 10410 行；BabelDOC 190 个 .py / 75257 行，含静态数据和辅助代码，不能据此估算必需移植代码量。
- 真实 BabelDOC PDF 模块包括段落、样式/公式、表格、布局、翻译、typesetting、fontmap 和输出合并。Rust 重写的重点是这些层之间的语义与对象关联。

## 2026-10-23 follow-up: deployment download cost
- The HTTP-400 repair cloud run reused its resource-stage cache, but the final runtime resource COPY executed after the changed binary COPY, so its cache was invalidated by the preceding layer. NAS pulled a new large resource blob despite the model hashes being unchanged. A future Docker-only optimization should place stable resource/license COPY layers before the frequently changing binary (or verify COPY --link support). Do not interrupt the current verified image pull or replace it with an unverified derivative while recovering the user failure.

## 2026-10-23 inline/code/reader root cause
- NAS user job now completed, 47 pages/764 translated blocks; its generated PDF itself reproduces page-22 overlap. This is not merely a viewer zoom effect.
- Native MuPDF divides the RoPE paragraph into overlapping blocks 12–15 at stacked fractions/subscripts; engine treats each block independently and allows their reflow areas to overlap. Same defect occurs in page-22 blocks 2/3 and 9/10. Layout model correctly detects each as one plain-text paragraph; engine ignores that paragraph grouping.
- Formula raster capture is based on rectangular bounds, which can include glyphs from adjacent visual rows; use exact glyph ownership while retaining formula vectors.
- Code fonts in real pages 5/15 are DejaVuSansMono. Engine protects math fonts only, so standalone code and inline identifiers can reach translation and lose syntax/layout. Preserve standalone monospaced blocks; protect inline code as original fragments.
- Reader uses nested 75vh PDF iframes, max-w-2xl/max-w-6xl and transform-scale on the outer container; browser PDF plugin owns inner scrolling/key events. Replace with Mozilla PDF.js page canvas + selectable text layer, application scrolling and keyboard control; expand reader area, add fullscreen/fit width and real zoom.

- Offline page-22 render now has a single flowing RoPE paragraph with fourteen original fragments and no body/formula overlap. Synthetic Chinese supplied by a local mock is explicitly geometry-only. Additional inspection found original fraction vector rules left behind after glyph removal; scoped covered-path redaction now removes only moved fragment paths, preserving crossing frame paths/images/text, covered by pixel regression.
- Whole source-code blocks must be separated before joining detector text regions; a broad plain-text detector region can include code plus prose. Added a guard and regression to preserve indentation/line positions even in that case.
- Workspace was moved by the user to Desktop/projects between sessions. Updated local test server path and restarted only the owned mock; an initial diagnostic retry failed solely because its old absolute log path no longer existed. NAS ZeroTier and LAN probes time out; no network settings were changed.
