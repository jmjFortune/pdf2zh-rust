# pdf2zh-next Rust 完整重写计划

日期：2026-10-22。范围是独立替代上游 pdf2zh-next/BabelDOC；当前 translator 项目代码与 NAS 已运行服务保持原状。

## 工作阶段

1. 上游能力、版本及内存基线核查 — complete
2. 明确 Rust / 原生库边界、比较实现方案 — complete；用户了解差异后改选 Rust＋成熟原生库，不再要求所有底层纯 Rust
3. 编写可审阅设计、功能对照表与验收标准 — complete；见 design.md
4. 设计自审与用户设计确认 — complete；用户回复“OK”
5. 独立目录建立 Rust 工程、先验证 PDF 核心链路 — complete；用户最新指示改在本机开发，Rust/MuPDF 构建、中文写回、公式/向量像素对照、原生布局推理已通过
6. 完成翻译、排版、导出、CLI 与 Web 服务 — in_progress；本机开发版已通过真实 Google、原生公式重排、三种导出路径、Web 预览下载和 worker 生命周期测试；完整上游兼容、OCR、表格及跨页语义仍未完成
7. NAS 容器验证、同输入质量与内存对比 — pending

## 约束

- 完整重写的最终产物不依赖 CPython、Gradio、pdf2zh-next 或 BabelDOC 的 Python 运行时。
- 不将仅 Rust 包装现有 Python 引擎称为完整重写。
- 用户随后明确要求“你先在本机做呗”，开发位置改为 `/Users/shiyun/Desktop/pdf2zh-rust`；允许本机源码/依赖/编译。NAS 原版不替换，Mac 不拉 Docker 镜像。
- 当前 NAS 容器不覆盖、不停止，沿用已有代理策略。
- 内存数字是实测或明确标注的目标；不得将减小待机内存等同于翻译峰值同比下降。
- 模型与字体仍是数据资产，是否可复用取决于许可和技术验证。
- 用户已允许复用 C/C++ PDF、字体和推理库；复用底层不等于 BabelDOC 的 Python 语义处理流程已经被替代。

## 已遇到问题

- 第一次合并读取输出过长被截断，后续按来源拆分并限制输出。
- 一次读取异步命令时未保存 session ID，误向不存在的 session 0 查询；已改为显式保存会话 ID 后查询，不影响 NAS 服务。
- ort.pyke.io 文档返回 403，后续改查项目仓库和 docs.rs 的官方 crate 文档。
- 两个自行拼接的深层 crate 文档 URL 无法打开，已从 crate 官方目录点击取得实际模块地址。

## 当前完成状态

设计已批准，本机开发版可运行，完整上游替代仍实施中。已验证路径与内存口径见 verification.md；尚未完成 NAS 同环境对比，不能宣称整体重写或峰值内存目标已经达成。
