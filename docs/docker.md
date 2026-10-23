# Docker development preview

The repository builds `linux/amd64` for the NAS on standard GitHub-hosted Ubuntu
24.04. This does not replace the original pdf2zh-next container and does not
establish full upstream feature parity.

## Build and publish

Open the repository's **Actions → Docker image** page. Source changes on
`agent/codex/rust-engine` trigger the workflow; it can also run manually. Pull
requests build and test without publishing. The job skips private repositories.

The image build runs the Rust release test suite with the real native model,
font and Linux ONNX Runtime. Translation API tests use a local mock, not a paid
API. After the final container passes resource hashes, native inference, HTTP
health and non-root checks, the workflow publishes:

- `ghcr.io/jmjfortune/pdf2zh-rust:sha-<first 12 commit characters>`
- `ghcr.io/jmjfortune/pdf2zh-rust:dev` for the current development branch

The first build compiles native dependencies. Cargo Chef and the exported Buildx
layer cache avoid repeating unchanged dependency work. Cache and download
performance are separate issues. No scheduled workflow, larger runner, NAS
credential or automatic deployment is configured.

Image/package visibility is separate from repository visibility. Check the GHCR
package's access settings after first publication. A private package requires
registry authentication on the NAS; a public package supports anonymous pulls.

First-run verification on 2026-10-23 passed: run 39885551001 published
`sha-8c5dbb4b2d68` and `dev`, and the registry accepted anonymous manifest access.
The 11-test Linux suite and final-container smoke checks passed. Exact revision
and digest are recorded in `verification.md`.

NAS deployment was subsequently verified on 2026-10-23 with the same image
digest, a separate named volume and a trusted private-interface binding. Live
Google translation, native inference, rendered preview/download and persistent
history/output across a restart passed. The single-page test took 29.59 seconds
with a 432.45 MiB sampled container working-set peak and about 2 MiB after worker
exit. This is a synthetic smoke test, not a large-document performance promise.
Host addresses and runtime settings remain in ignored local deployment records.

## Run on a Linux amd64 machine

Place `compose.yaml` in its own directory, then run:

```sh
docker compose pull
docker compose up -d
docker compose logs -f
```

The default host binding is `127.0.0.1:7861`. To make it reachable through a
specific trusted NAS interface, set `PDF2ZH_BIND_ADDRESS` to that interface's IP
in a local `.env` beside the Compose file. `PDF2ZH_PORT` changes the host port;
the original service's port 7860 is independent. The development Web UI has no
authentication, so keep its binding on a trusted interface.

The named volume `pdf2zh-data` persists task history, input/output PDFs and the
bounded translation cache under `/data`. Do not use `docker compose down -v` to
update. Update with `pull` followed by `up -d`. Set `PDF2ZH_IMAGE` to a specific
commit tag for repeatable deployment or to return to a previous built version.

The process runs as UID/GID 10001, and the image provides an owned `/data`
directory for Docker's volume initialization. A bind-mounted host directory
instead needs appropriate write permissions. The service honors conventional
HTTP proxy environment variables; configure those for the NAS network only if
needed. Image pulls use the Docker daemon's network configuration separately.

## Resources and runtime

The multi-stage Dockerfile downloads the native ONNX Runtime archive from its
official release and checks SHA-256. Model/font files and license texts use the
hashes and URLs in `assets/manifest.json`; model/font checks use SHA3-256. The
runtime image includes those assets, ONNX notices and the model/font licenses.
There is no Python interpreter, Gradio, CPython extension or Python dependency
installer in the application image.

Model inference peaked around 1.3 GiB in the earlier Mac process-RSS measurement;
the NAS single-page cgroup sample was different, as reported above. These are
different platforms and metrics. The separate analyzer exits before remote
translation. Font subsetting, OCR, full table translation and other gaps remain
as documented in `verification.md`.

The Dockerfile can be built by another Linux builder with `docker buildx build
--platform linux/amd64 --load -t pdf2zh-rust:local .`. Docker builds and image
downloads have not been performed on the development Mac.

## Font repair image target

The cloud workflow currently uses `runtime-patch`. It compiles/tests the current
source and reuses the pinned, previously verified runtime/model image, adding the
GoNotoKurrent fallback font, OFL notice, current manifest and current native binary.
This keeps NAS repair downloads small instead of replacing unchanged model layers.
The complete Debian-based image remains available as `--target runtime`. Rebase
the pinned repair image or switch the workflow back to `runtime` when changing
native runtime dependencies, the base system, model, or primary font; do not keep
using the patch baseline indefinitely for those changes. Final resource hashes,
real inference, service, UID checks still run on the exact image published.
