# The harness image: Stalwart, plus Stalwart's own CLI, which the entrypoint configures it with.
#
# Both pinned by multi-arch index digest (linux/amd64 + linux/arm64). Bump deliberately: re-resolve
# the digest (`docker buildx imagetools inspect <image>:<tag>`, the top-level `Digest:`) and update
# the version in the tag. The CLI is a static binary, so it runs in the server's image as copied.
FROM ghcr.io/stalwartlabs/cli:1.0.12@sha256:fe199affac1d120a8c200ef39ae629765a2976270e0453575c1caf906ee15b52 AS cli

FROM stalwartlabs/stalwart:v0.16.22@sha256:388dcb75a70727c5b551249a6d34b1f1321294852489e4fa3a4e6be698b7c4f0

COPY --from=cli /usr/local/bin/stalwart-cli /usr/local/bin/stalwart-cli
