# syntax=docker/dockerfile:1

FROM node:26-trixie-slim@sha256:e6bedef217677b276c591795e169840edd9096dbf13f29df8ef8d1856b8820bc AS frontend
WORKDIR /src/frontend
COPY frontend/ ./
# Node 25+ no longer bundles corepack, so install the pnpm version pinned in packageManager.
RUN if [ -f package.json ]; then npm install -g "$(node -p 'require("./package.json").packageManager')" && pnpm install --frozen-lockfile && pnpm build; else mkdir -p dist; fi

FROM rust:1-alpine@sha256:8d0e82abfdf9a38071e07cabc5afba4ddbe077631a80e4e742a760c38beadc33 AS backend
RUN apk add --no-cache musl-dev build-base
WORKDIR /src/backend
COPY backend/ ./
ARG SOABOX_GIT_SHA=unknown
ENV SOABOX_GIT_SHA=${SOABOX_GIT_SHA}
RUN cargo build --release --locked --bin soabox && mkdir -p /out/data

# Static musl binary on a base with no C library or OpenSSL.
FROM gcr.io/distroless/static-debian13:nonroot@sha256:aafe993665bf6200db38a02c11a9351e782a84b6a45e7f33b6695638c88e3931
WORKDIR /app
COPY --from=backend /src/backend/target/release/soabox /usr/local/bin/soabox
COPY --from=frontend /src/frontend/dist /app/static
COPY --from=backend --chown=1001:1001 /out/data /app/data
ENV SOABOX_STATIC_DIR=/app/static \
    SOABOX_DATABASE_URL=sqlite:///app/data/soabox.db \
    SOABOX_BIND=0.0.0.0:8080
USER 1001:1001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/soabox"]
CMD ["serve"]
