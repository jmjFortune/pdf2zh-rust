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

## 2026-10-23 NAS deployment smoke

- Hardware: Intel Celeron J1900 at 1.99 GHz, approximately 8 GB RAM, Linux amd64;
  Docker 28.5.2 and Compose 2.40.3. Image manifest digest is the same verified
  cloud digest above; container image ID is
  `sha256:252debbe5d41cc62692a8f554c3271e17ece1d48f6520977d222ee07778b62a0`.
- Deployment is an independent Compose project, container `pdf2zh-rust`, UID/GID
  10001, private-interface port 7861 and named volume `pdf2zh-rust_pdf2zh-data`.
  Restart policy is `unless-stopped`. Host-specific runtime files are outside
  this repository; addresses, reports, samples and screenshots stay in ignored
  local artifacts. No Docker image was downloaded or built on Mac.
- Large-layer pull retries eventually succeeded without Docker daemon/network
  changes. The model and font passed the embedded hash checks on NAS.
- The first Google smoke job failed at the network stage. Container and host
  curl reproduced TLS timeouts. Independent proxy delay tests returned a 504
  for the existing default selector but HTTP 200/438 ms through the existing
  registry egress. A backed-up, validated rule scoped to
  `translate.googleapis.com` now selects that working egress. Hot reload
  preserved all prior selector values and the proxy container's start time;
  proxy mode, TUN and Docker daemon settings were not changed.
- A fresh synthetic one-page task then passed real native layout inference,
  live Google English→Chinese, side-by-side export, native PDF reopening/text
  extraction, PDF download and PNG preview. Task completion took 29.59 seconds.
  Output is 9,026,930 bytes and the preview is 20,997 bytes. The browser showed
  completed history and the bilingual page; the PNG was visually inspected.
  This was a warm-filesystem second attempt, not a cold benchmark. The first
  failed synthetic task remains in test history as diagnostic evidence.
- Host-side cgroup v2 sampling at approximately 100 ms (318 samples) recorded
  working set as `memory.current - inactive_file`: baseline 1.72 MiB, sampled
  peak 432.45 MiB, and 1.94 MiB after completion. Raw `memory.current` peak was
  432.84 MiB. Docker stats independently reported 1.934 MiB after the task and
  1.359 MiB after restart. The native analyzer and translation workers had exited;
  the idle container held docker-init and the Rust service only. The Python
  measurement script runs on the host, not as an application dependency.
- Restart verification ran only while all tasks were terminal. The new Rust
  container returned healthy; completed task history and the PDF SHA-256 survived
  on the named volume. No lifecycle operation targeted the old upstream service.
- Text-layer follow-up: native extraction of this visibly correct page returned
  the full translated sentence plus a duplicate trailing `变。` fragment and
  whitespace. The cause is not yet established. Exact full-paragraph extraction
  across all PDF consumers is therefore not claimed; keep this case for a later
  PDF text-layer regression fix. Existing Unicode-presence tests alone do not
  cover this defect.
- Private evidence: ignored `artifacts/nas-smoke-report.json`,
  `artifacts/nas-deployment-record.json`, `artifacts/nas-preview.png` and
  `artifacts/nas-deployed.jpg`; raw samples/restart record remain in the NAS
  deployment's smoke directory. This is one small fixture, not an upstream A/B
  comparison or a bound for larger/scanned documents.

## Remaining coverage

Full upstream configuration/provider parity, cross-page paragraphs, complete table text translation, OCR, broad multilingual shaping, exact text-layer roundtrips, font subsetting, production hardening and a controlled upstream comparison are not complete. The development preview and README state these limits explicitly.


## 2026-10-23 first-page HTTP 400 investigation

- Real NAS failure for a 47-page input was at `translate` page 1, zero completed blocks. Only the previous one-page smoke translation was in the shared cache. Full-document layout analysis had consumed the time and looked like nearly completed translation in the old UI.
- The old code discarded the HTTP response body and removed credentials at terminal state. User confirms a key was supplied. Therefore the exact original rejection cannot be reconstructed from retained history. Missing OpenCode session/client identity is a concrete compatibility defect and plausible cause, supported by [OpenCode documentation](https://dev.opencode.ai/v2/docs/console/go) and [official-repository report #47756](https://github.com/anomalyco/opencode/issues/47756); it is not claimed as confirmed against the user's key.
- Local regression checks cover structured/redacted/bounded provider diagnostics, permanent-vs-transient retry selection, application/session headers, a two-page native-layout task rejected on page 1, retained-file retry into a distinct completed history record, and private-settings deletion. Full local suite and cloud/NAS deployment verification recorded after execution below.
- Anonymous NAS diagnostic probes did not reproduce the authenticated failure (one generic client got edge HTTP 403; later honest-user-agent requests timed out). These are not evidence of an invalid user key or successful OpenCode integration. No credentials were read from unrelated applications or requested in chat.

- Fresh local verification: 12 tests passed (7 unit, 4 PDF, 1 process/HTTP lifecycle), release build succeeded; JS parsed, fmt and diff checks passed. The expanded lifecycle regression failed against restored HEAD engine/translator because the old client omitted required identity headers; restoring the patch and running the full suite passed. Logs of the intentional before-fix failure are ignored under artifacts.

- Cloud run [39916128410](https://github.com/jmjFortune/pdf2zh-rust/actions/runs/39916128410) succeeded: 12 Linux tests and final image resource/inference/HTTP/non-root checks passed. Published/pulled digest `sha256:d916e2f53f107e47e3b8cdb9b58e3d2ff1fc36355d2bb69699a0a644654aaa48`, source `80d499bf02bc1377c7813de3a110e4ed6674c9d5`.
- NAS idle-only upgrade succeeded, healthy. Three original history records compared equal, original input hashes unchanged, existing volume retained. NAS live native-worker regression using a clearly named synthetic PDF and LAN mock completed in 56.48 s: one non-retried 400 with sanitized diagnostic, same-input retry completed with downloadable PDF, original failure retained, credentials files removed. Two HTTP requests total; identity/session headers present. This validates infrastructure and behavior, not real OpenCode credentials/translation.
- Browser confirms the original user failure now displays translate page 1/47 and zero completed blocks alongside retained-file retry. Original task cannot recover its discarded provider body or Key; real OpenCode validation awaits the user's key entered in the page. Screenshot/report private under ignored artifacts and NAS diagnostic directory. The 84.14 MiB resource blob needed a resumed download; no proxy/daemon setting was changed during this repair.
