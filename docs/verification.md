# Verification record

## 2026-10-23 local development

- Platform: Apple Silicon, Rust 1.96.0. `mupdf` 0.8.0; `ort` 2.0.0-rc.15, native ONNX Runtime 1.31.0. Cargo.lock records the resolved dependencies.
- Native build and tests passed. The latest suite has 11 tests: six unit tests, four PDF integration tests, and one HTTP/process lifecycle integration test. The HTTP fixture is explicitly a mock OpenAI-compatible translator; it is not presented as live translation quality evidence.
- Live Google translation succeeded on the synthetic public test paragraph, with mono/side-by-side output rendered and visually inspected. The public `https://arxiv.org/pdf/1706.03762` was used for selected-page regression checks; source PDF and translations stay in ignored artifacts.
- Real native layout inference produced class-labeled detections using the pre-existing model. No Python interpreter is called by the application.
- A real-paper inline-math defect was found and fixed: whole paragraphs now carry exact formula markers; original formulas are moved as native PDF forms. Text outside each captured formula is removed before clipping, preventing invisible copies of whole source paragraphs.
- Source Han font aliases initially yielded compatibility ideographs such as `器` when copying visually correct `器`. PDF ActualText now preserves exact translated Unicode. The real-paper extraction found seven exact occurrences of `编码器`, and one original `LayerNorm` formula, after this fix.
- Latest full suite passed: six unit, four PDF integration and one lifecycle integration test. The lifecycle test now includes real native model analysis, exact Greek inline-formula extraction in reading order, queued and blocked cancellation, restart recovery, streamed PNG preview and removal of private job snapshots. Release build passed after the final UI stage-label change.
- Final browser check passed with the standalone release bundle launched from `/private/tmp`: synthetic upload → native layout → live Google → vector-preserving side-by-side PDF → visible PNG preview → browser download. The downloaded file has a PDF-1.7 header. Screenshot: ignored `artifacts/local-web-preview.jpg`. No Python interpreter is involved in these stages.
- Model inference now runs in a separate Rust analyzer process that exits before translation requests. Optional lifecycle profiling (`PDF2ZH_PROFILE_MEMORY=1 cargo test --test service_lifecycle`) sampled process RSS: analyzer peak 1296.6 MiB, translation-worker peak 71.9 MiB, service peak 11.9 MiB. During the first two seconds of a deliberately delayed local mock request, analyzer RSS was zero, worker RSS 29.3 MiB and service RSS 11.4 MiB. This checks allocation lifetime, not real-provider throughput or overall peak reduction. Samples are in ignored `artifacts/native-stage-memory.json`.
- After the final browser job completed, release-service RSS samples were approximately 5–7 MiB on this Mac. RSS varies with macOS memory management. These process figures are not a controlled comparison to the NAS original's 334.4 MiB cgroup figure; NAS validation remains pending.
- Main release executable is approximately 12.0 MiB. Its direct `otool -L` dependencies are system `libiconv` and `libSystem`; ONNX Runtime is loaded dynamically. The standalone folder contains the executable, model, font and native runtime; resource hashes were verified and standalone translation passed with Python/resource environment variables unset. Native runtime assets remain OS/architecture-specific.
- Bing returned HTTP 401 in live development checks, including the cookie-session implementation. That adapter remains experimental and is omitted from the Web selector; other unverified adapters are not claimed to work live.

## 2026-10-23 Linux amd64 cloud image

- Public repository: https://github.com/jmjFortune/pdf2zh-rust . User explicitly
  selected the Rust repository and authorized public visibility after checking
  that only the separate translator repository had previously been public.
- Source/image revision: `8c5dbb4b2d68c0d4d1cf7215c8049cd6cb5ef99d`.
- GitHub-hosted Ubuntu 24.04 run
  [39885551001](https://github.com/jmjFortune/pdf2zh-rust/actions/runs/39885551001)
  succeeded; job duration 9m34s. No Docker images were downloaded or built on Mac.
- Docker's Linux release suite passed six unit tests, four PDF integration tests
  and one real HTTP/process lifecycle integration test. That last test took
  8.55s and used native Linux ONNX inference with a local mock translator.
- The final runtime container passed font/model hash verification, native model
  inference with nonempty detections, HTTP health and UI response, and UID 10001.
- Published image: `ghcr.io/jmjfortune/pdf2zh-rust:sha-8c5dbb4b2d68`; `dev` matched
  that image at verification time. Both publish logs report manifest digest
  `sha256:2dfb6624948c13d3eef6c6f6ebee76a08b6810c1d7b85673ae0dd122935222be`.
- An unauthenticated registry-token request followed by a manifest request
  returned HTTP 200 with the same digest. No local Docker pull was performed.
  This verifies anonymous registry access independently of the separate GitHub
  package-metadata API, which rejected the current OAuth token for missing
  read:packages scope. No credential permissions were changed.
- The image contains pinned native runtime, model/font data, license texts,
  application license and NOTICE. Release tests run before publication. The
  workflow exports Buildx layer caches and does not upload a build-record artifact.
- This verifies Linux image operation on the cloud runner. It is not a NAS
  deployment, NAS memory benchmark or full upstream quality/parity acceptance.

## Remaining coverage

Full upstream configuration/provider parity, cross-page paragraphs, complete table text translation, OCR, broad multilingual shaping, font subsetting, production packaging and NAS deployment are not complete. The development preview and README state these limits explicitly.
