FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN rustup target add aarch64-unknown-linux-musl \
 && cargo clippy -p rubix-kube --lib --test host_network --example prepare_host_network --target aarch64-unknown-linux-musl --locked -- -D warnings \
 && cargo test -p rubix-kube --test host_network --target aarch64-unknown-linux-musl --locked --release --no-run \
 && cargo build -p rubix-kube --example prepare_host_network --target aarch64-unknown-linux-musl --locked --release \
 && mkdir /out \
 && cp target/aarch64-unknown-linux-musl/release/examples/prepare_host_network /out/prepare_host_network \
 && for executable in target/aarch64-unknown-linux-musl/release/deps/host_network-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/host_network; fi; done \
 && test -x /out/host_network
ARG QUALIFICATION_NONCE
RUN test -n "$QUALIFICATION_NONCE" && echo "RUBIX_BUILD_BIND_BEGIN $QUALIFICATION_NONCE" && sha256sum /out/prepare_host_network /out/host_network && echo "RUBIX_BUILD_BIND_END $QUALIFICATION_NONCE"
FROM alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
COPY --from=build /out/ /out/
USER 65532:65532
ENTRYPOINT []
