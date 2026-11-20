# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS frontend
WORKDIR /src/frontend
RUN corepack enable
COPY frontend/ ./
RUN if [ -f package.json ]; then pnpm install --frozen-lockfile && pnpm build; else mkdir -p dist; fi

FROM rust:1-bookworm AS backend
WORKDIR /src/backend
COPY backend/ ./
ARG SOABOX_GIT_SHA=unknown
ENV SOABOX_GIT_SHA=${SOABOX_GIT_SHA}
RUN cargo build --release --locked --bin soabox && mkdir -p /out/data

FROM gcr.io/distroless/cc-debian12:nonroot
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
