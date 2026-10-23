# Docker cloud build

2026-10-23. User approved GitHub-hosted builds, then explicitly selected
`pdf2zh-rust` and authorized changing it to public after the repository mismatch
was identified. The application remains a development preview.

## Selected design

- GitHub Actions on standard Ubuntu 24.04 builds native `linux/amd64` images for
  the existing NAS architecture. Public-repository-only job guard; no paid or
  self-hosted runner and no local Docker image download on the Mac.
- Multi-stage Docker build compiles Rust/MuPDF, downloads the native ONNX Runtime
  archive with pinned SHA-256, and verifies existing model/font SHA3 metadata.
  Cargo Chef caches native/Rust dependencies; Buildx exports layer caches to GHA.
- Run all tests in the Linux build with real native resources and a local mock
  translator. Smoke-test the final non-root image, HTTP health, resource hashes
  and native inference before publishing it to GHCR.
- Publish commit-tagged images and a `dev` tag on the development branch. A
  Compose file mounts persistent data and exposes the Web UI on host loopback by
  default; trusted NAS-interface binding is an explicit configuration option.
- Source push and manual triggers build; pull requests validate without registry
  login or publishing. No scheduled builds, NAS credentials or automatic NAS
  replacement. Dependencies/licenses ship with the image; user PDFs, local
  assets, secrets and build products stay out of the source/build context.

## Alternatives considered

Local Mac builds need an amd64 builder/emulation and would download images here;
NAS builds consume the device's compute and require more remote setup. The
approved cloud runner keeps those builds separate from NAS operation. Supporting
arm64 in the download metadata is useful for later work, but this first workflow
only claims amd64 coverage.

## Execution and acceptance

1. Confirm public visibility and current source checkpoint — complete.
2. Add pinned resource downloads, Dockerfile, Compose and Actions workflow — complete.
3. Validate shell/YAML/Compose, run current native tests and inspect the diff — complete; actionlint 1.7.12, sh -n, docker compose config, JSON and git diff checks passed; all 11 current Rust tests passed locally.
4. Push on agent/codex/rust-engine, verify remote SHA and follow the cloud run — complete; source checkpoint 8c5dbb4, run 39885551001 succeeded.
5. Verify published image and record Linux evidence/remaining NAS work — complete; anonymous GHCR manifest retrieval returned HTTP 200, matching the published digest. NAS deployment remains separate.

Existing build caches do not eliminate registry/network latency. An uploaded
image is not a NAS deployment, and passing the current suite does not establish
full compatibility with upstream.

## NAS deployment — 2026-10-23

User requested deployment after the image passed cloud checks. NAS is Linux
amd64 with Docker Compose v2, sufficient available memory/disk, and unused port
7861. Deploy the verified image by digest into a separate Compose project and
named data volume; preserve the existing upstream container's current state.
Bind only the trusted private NAS interface. Reuse the existing host proxy for
translation requests, without changing daemon proxy, mirrors or TUN settings.

1. Inspect NAS, existing containers, instructions and network — complete.
2. Pull verified digest on NAS and write private runtime Compose settings — complete; transient large-layer retries resolved without changing Docker daemon settings.
3. Start container, verify UID/assets/HTTP from NAS and client — complete; healthy, UID 10001, resources valid, client HTTP 200.
4. Run a synthetic live translation, preview/download and observe cgroup memory — complete; native inference, live Google, Chinese/formula extraction, PDF/PNG responses and visual preview passed. The default proxy-egress timeout was resolved by a single-domain rule after independent route comparison; existing selectors and proxy process remained intact.
5. Record outcome and remaining scope — complete; results are in verification.md. A restart of the idle Rust container preserved history and the output hash. Documentation push is checked separately against remote HEAD at handoff.

Host-specific addresses/configuration and test artifacts stay out of Git. The
test PDF is synthetic and contains no user data; Google is the existing free
development provider. A deployment smoke test is not full upstream quality or a
controlled performance comparison.
