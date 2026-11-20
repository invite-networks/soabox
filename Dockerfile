# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim@sha256:e9f565d32859f0cbfb52125eafe8be39390f24dac302dc3cdf566e8ba3ab21ee AS frontend
WORKDIR /src/frontend
RUN corepack enable
COPY frontend/ ./
RUN if [ -f package.json ]; then pnpm install --frozen-lockfile && pnpm build; else mkdir -p dist; fi

FROM rust:1-alpine@sha256:8d0e82abfdf9a38071e07cabc5afba4ddbe077631a80e4e742a760c38beadc33 AS backend
RUN apk add --no-cache musl-dev build-base
WORKDIR /src/backend
COPY backend/ ./
ARG SOABOX_GIT_SHA=unknown
ENV SOABOX_GIT_SHA=${SOABOX_GIT_SHA}
RUN cargo build --release --locked --bin soabox && mkdir -p /out/data

# Static musl binary on a base with no C library or OpenSSL.
FROM gcr.io/distroless/static-debian12:nonroot@sha256:afa5c872c891853ca7fcf1f12c3edb23f7eeef36189728842dd51042ff57f7ab
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
