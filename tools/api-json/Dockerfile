FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS base
RUN apt-get update && apt-get install -y --no-install-recommends openssl curl sqlite3 ca-certificates && rm -rf /var/lib/apt/lists/*
FROM base AS build
WORKDIR /source
COPY . .
ENV RUBIX_DISPOSABLE_BUILD=api-json
RUN cargo build --locked --release -p rubix-dev --bin rubix-api-json && target/release/rubix-api-json stage-sources
FROM base
WORKDIR /experiment
COPY --from=build /source/target/release/rubix-api-json /fixture
COPY --from=build /experiment/sources /experiment/sources
COPY tools/api-json/inputs.json /experiment/inputs.json
ENV RUBIX_DISPOSABLE_BUILD=api-json RUBIX_DISPOSABLE_FIXTURE=api-json
RUN /fixture fetch && mkdir /evidence /state && chown -R 65532:65532 /evidence /state && chmod 0700 /evidence /state
USER 65532:65532
ENTRYPOINT ["/fixture", "runtime"]
