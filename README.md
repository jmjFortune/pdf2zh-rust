# pdf2zh-rust

Independent rewrite of PDFMathTranslate-next/BabelDOC with Rust application code and native MuPDF/ONNX Runtime. No Python interpreter is used by the application.

Implementation in progress. This is not yet a complete replacement for upstream. Development and tests run locally on Apple Silicon; the Linux amd64 Docker preview is also deployed and smoke-tested on NAS.

The resident HTTP service starts a separate Rust worker for each translation. The worker starts another short-lived Rust process for layout inference; that process exits before remote translation begins, releasing the model's allocations. The translation worker exits when its task finishes.

## Build

Rust 1.92 or later, Clang and Make are required. MuPDF is compiled by its Rust binding. ONNX Runtime is loaded dynamically; set `ORT_DYLIB_PATH` to the matching native library (API 23 or newer).

```sh
cargo build -j 2
```

## Run locally

```sh
npm --prefix web ci
PDF2ZH_STATIC_EXPORT=1 npm --prefix web run build
cargo build --release --locked -j 2
./target/release/pdf2zh-rust resources status
./target/release/pdf2zh-rust serve --listen 127.0.0.1:7861 --data-dir data
```

Open http://127.0.0.1:7861/. The existing translator frontend is compiled to static files and served by Rust; Node is only a build dependency. No Python process is started. Uploaded PDFs, task state and the bounded translation cache live in `data/`, which is ignored by Git. A task's credentials/password snapshot is mode 0600 and is deleted after completion or cancellation. The translator frontend's explicitly saved provider keys remain in private `ui-settings.json`; reads return only whether a key exists. Public task metadata preserves the selected settings without credentials.

Fonts and the layout model are initially reused read-only from `~/.cache/babeldoc`. `PDF2ZH_ASSETS` can select a different resource directory and `PDF2ZH_FONT` can override the font. Prepare a standalone folder explicitly:

```sh
mkdir -p dist/pdf2zh-rust
./target/release/pdf2zh-rust resources prepare --output dist/pdf2zh-rust/assets
cp target/release/pdf2zh-rust LICENSE NOTICE dist/pdf2zh-rust/
cp scripts/Run.command dist/pdf2zh-rust/
cp -R web/out dist/pdf2zh-rust/web
chmod +x dist/pdf2zh-rust/Run.command
./dist/pdf2zh-rust/pdf2zh-rust serve --data-dir data
```

`resources prepare` checks the font and model against pinned SHA3-256 values. It copies existing valid resources and the native runtime; it does not download unless `--download` is explicitly supplied. The bundled executable discovers adjacent assets, so the original Python installation is unnecessary. The native runtime must be supplied for the target OS/architecture.

## Docker cloud builds

The **Docker image** GitHub Actions workflow builds and tests `linux/amd64` on a
standard public-repository Ubuntu runner, then publishes the checked image to
GHCR. Cargo Chef and Buildx cache dependencies; the Mac does not build/download
Docker images. See [Docker instructions](docs/docker.md) for image tags, Compose,
resource verification and persistent data. First-run cloud status is recorded in
the verification log; adding the workflow alone does not mean a build passed.

## CLI

```sh
./target/release/pdf2zh-rust translate input.pdf translated.pdf --source en --target zh-CN
./target/release/pdf2zh-rust translate input.pdf bilingual.pdf --format side-by-side --pages 1-3
./target/release/pdf2zh-rust translate input.pdf alternating.pdf --format interleaved
./target/release/pdf2zh-rust translate input.pdf result.pdf --config settings.json
```

Google is the default. OpenAI-compatible services, Ollama, DeepL, DeepLX, Bing, Azure Translator, Azure OpenAI, Dify and AnythingLLM have adapters. See [API and configuration](docs/api.md). Google passed live checks; the OpenAI-compatible path is covered by a local HTTP contract test. Bing returned HTTP 401 in live checks and is not offered in the Web selector. Other adapters need provider-specific live verification and do not imply complete upstream compatibility.

## Verification and current boundary

```sh
cargo test --locked -j 2
```

Tests include exact searchable Chinese Unicode, source-text removal, protected formula/vector pixels, formula relocation without hidden duplicate paragraphs, all four rotations with crop boxes, links/outlines in dual output, page validation, formula-marker validation, streaming preview, real worker and model-analysis processes, queued/blocked cancellation, and restart recovery. PDF tests require the prepared font and model.

Live checks used synthetic PDFs and selected pages of the public paper *Attention Is All You Need*. The inline-formula path translates whole paragraphs with validated markers, then places original formula glyphs through clipped native PDF forms. It is not a guarantee of quality on every PDF. Current gaps include full upstream option/provider parity, cross-page paragraph semantics, complete table-text translation, OCR and broad language/font-shaping coverage. NAS smoke checks passed live Google translation, preview/download and persistent history/output across a container restart; see the verification record for measurements and text-extraction limits.

Development memory observations are recorded in `docs/verification.md`. Mac process RSS and NAS cgroup memory are different metrics; they must not be treated as a controlled speed or memory benchmark.
