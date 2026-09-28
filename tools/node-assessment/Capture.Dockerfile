FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo build -p rubix-dev --bin rubix-node-fixture --release --locked && cargo test -p rubix-kube --test host_preflight --test iptables_probe --locked --no-run && cargo build -p rubix-kube --example assess_host --locked && mkdir /out && for suite in host_preflight iptables_probe; do count=0; for executable in target/debug/deps/$suite-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/$suite; count=$((count + 1)); fi; done; test "$count" -eq 1 || exit 1; done && cp target/debug/examples/assess_host /out/assess_host && cp target/release/rubix-node-fixture /out/node-fixture
ARG QUALIFICATION_NONCE
RUN test -n "$QUALIFICATION_NONCE" && echo "RUBIX_BUILD_BIND_BEGIN $QUALIFICATION_NONCE" && sha256sum /out/host_preflight /out/iptables_probe /out/assess_host /out/node-fixture && echo "RUBIX_BUILD_BIND_END $QUALIFICATION_NONCE"
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COPY --from=build /out/ /out/
COPY --from=build /out/node-fixture /node-fixture
COPY tools/node-assessment/iptables.sh /iptables.sh
RUN test -L /sbin && rm /sbin && mkdir /sbin
USER 65532:65532
ENTRYPOINT []
