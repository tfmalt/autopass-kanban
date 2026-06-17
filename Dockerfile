# Build context is this directory:
#   docker build .

FROM node:24-bookworm-slim AS web-build

WORKDIR /src
COPY web/package*.json ./web/
RUN npm --prefix web ci
COPY web ./web
RUN npm --prefix web run build

FROM rust:slim-bookworm AS kanban-build

WORKDIR /src
ENV CARGO_TARGET_DIR=/src/target

COPY . .
COPY --from=web-build /src/web/dist ./web/dist
RUN cargo build --locked --release -p kanban-cli --manifest-path Cargo.toml

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

COPY --from=kanban-build /src/target/release/kanban /usr/local/bin/kanban
RUN ln -s /usr/local/bin/kanban /usr/local/bin/kb
COPY entrypoint.sh /usr/local/bin/kanban-entrypoint.sh
RUN chmod +x /usr/local/bin/kanban-entrypoint.sh

ENV KANBAN_WEB_HOST=0.0.0.0
ENV KANBAN_WEB_PORT=3000
ENV KANBAN_REPO_ROOT=/repo

EXPOSE 3000
WORKDIR /repo

ENTRYPOINT ["/usr/local/bin/kanban-entrypoint.sh"]
