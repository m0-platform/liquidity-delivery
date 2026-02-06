FROM debian:bookworm-slim

ARG SINK_SQL_VERSION=v4.12.0

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

RUN curl -fsSL "https://github.com/streamingfast/substreams-sink-sql/releases/download/${SINK_SQL_VERSION}/substreams-sink-sql_linux_x86_64.tar.gz" | \
    tar -xz -C /usr/local/bin substreams-sink-sql

ENV DSN=""
ENV SUBSTREAMS_API_TOKEN=""
ENV PACKAGE="svm-orderbook@v0.1.3"

ENTRYPOINT ["/bin/sh", "-c", "substreams-sink-sql from-proto \"$DSN\" \"$PACKAGE\""]
