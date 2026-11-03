# The harness image: Stalwart, plus Stalwart's own CLI, which the entrypoint configures it with.
#
# Both pinned by multi-arch index digest (linux/amd64 + linux/arm64). Bump deliberately: re-resolve
# the digest (`docker buildx imagetools inspect <image>:<tag>`, the top-level `Digest:`) and update
# the version in the tag. The CLI is a static binary, so it runs in the server's image as copied.
#
# The two move together (renovate.json): the CLI reads the server's own schema, and one too old to
# parse it fails before the entrypoint has bootstrapped anything.
FROM ghcr.io/stalwartlabs/cli:1.0.14@sha256:5b70732cb69cdab3ff0894c501544cf0bfa180fa75bcd0eeba4eebe04c1064e1 AS cli

FROM stalwartlabs/stalwart:v1.0.2@sha256:154e55adecadfcc4a67044b9b94d482a20286e1101c97a2b322224bb596053d5

COPY --from=cli /usr/local/bin/stalwart-cli /usr/local/bin/stalwart-cli

# Where the sign-in server mints its HTTPS front's certificate (entrypoint.sh). It exists in the
# image, owned by the server's user, because a named volume first mounted here copies that
# ownership; one mounted at a path the image lacks is root's, and the server cannot write to it.
USER root
RUN mkdir -p /harness/front-tls && chown stalwart:stalwart /harness/front-tls
USER stalwart
