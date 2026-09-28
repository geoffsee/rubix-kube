FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS base
RUN apt-get update && apt-get install -y --no-install-recommends openssl curl sqlite3 ca-certificates && rm -rf /var/lib/apt/lists/*
FROM base AS build
WORKDIR /source
COPY . .
ENV RUBIX_DISPOSABLE_BUILD=component-boundary
RUN cargo build --locked --release -p rubix-dev --bin rubix-component-boundary && target/release/rubix-component-boundary stage-sources
FROM base
WORKDIR /experiment
COPY --from=build /source/target/release/rubix-component-boundary /fixture
COPY --from=build /experiment/sources /experiment/sources
COPY experiments/component-boundary/inputs.json /experiment/inputs.json
ENV RUBIX_DISPOSABLE_BUILD=component-boundary RUBIX_DISPOSABLE_FIXTURE=component-boundary
RUN /fixture fetch && mkdir /evidence /state && chown -R 65532:65532 /evidence /state && chmod 0700 /evidence /state
USER 65532:65532
ENTRYPOINT ["/fixture", "runtime"]
