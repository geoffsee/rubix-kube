FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
ARG QUALIFICATION_NONCE
WORKDIR /source
COPY . .
RUN mkdir /tmp/musl-pins \
 && cd /tmp/musl-pins \
 && for package in musl musl-dev musl-tools; do curl --fail --silent --show-error --location --proto =https --proto-redir =https --max-time 60 --max-filesize 600000 --output "${package}_1.2.3-1_arm64.deb" "https://deb.debian.org/debian/pool/main/m/musl/${package}_1.2.3-1_arm64.deb"; done \
 && sha256sum -c /source/tools/node-constrained/build-packages.sha256 \
 && dpkg -i ./*.deb \
 && rm -rf /tmp/musl-pins
ENV CC_aarch64_unknown_linux_musl=musl-gcc
RUN rustup target add aarch64-unknown-linux-musl \
 && cargo clippy -p rubix-kube --lib --test host_preparation --test host_network --example prepare_node_host --target aarch64-unknown-linux-musl --locked -- -D warnings \
 && cargo test -p rubix-kube --lib --test host_preparation --test host_network --target aarch64-unknown-linux-musl --locked --release --no-run \
 && printf 'POLICY_TESTS_BEGIN\n' \
 && cargo test -p rubix-kube --example prepare_node_host --target aarch64-unknown-linux-musl --locked --release \
 && printf 'POLICY_TESTS_END\n' \
 && cargo build -p rubix-kube --example prepare_node_host --target aarch64-unknown-linux-musl --locked --release \
 && cargo clippy -p rubix-dev --bin rubix-constrained-guest --target aarch64-unknown-linux-musl --locked -- -D warnings \
 && cargo build -p rubix-dev --bin rubix-constrained-guest --target aarch64-unknown-linux-musl --locked --release \
 && mkdir /out \
 && cp target/aarch64-unknown-linux-musl/release/examples/prepare_node_host /out/prepare_node_host \
 && for name in rubix_kube host_preparation host_network; do count=0; for executable in target/aarch64-unknown-linux-musl/release/deps/$name-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then if "$executable" --list 2>/dev/null | grep -q ': test$'; then cp "$executable" /out/$name; count=$((count+1)); fi; fi; done; test "$count" = 1; done \
 && cp target/aarch64-unknown-linux-musl/release/rubix-constrained-guest /out/constrained-guest \
 && printf 'RUBIX_BUILD_BIND_BEGIN %s\n' "$QUALIFICATION_NONCE" \
 && sha256sum /out/prepare_node_host /out/rubix_kube /out/host_preparation /out/host_network /out/constrained-guest \
 && printf 'RUBIX_BUILD_BIND_END %s\n' "$QUALIFICATION_NONCE"
FROM alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
COPY --from=build /out/ /out/
USER 65532:65532
ENTRYPOINT []
