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
- 没做完 / 已知问题：首轮 Linux 镜像构建和 GHCR 可拉取性待云端验证；NAS 未部署。上游完整功能覆盖仍待完成，当前为开发版。
- 下一个接手的人先看：docker-build-plan.md、docker.md 和 Actions 的 Docker image 日志；构建自动触发范围限定源码和构建相关路径，无定时任务、付费 runner 或 NAS 自动部署。
