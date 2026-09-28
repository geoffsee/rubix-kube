FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo build -p rubix-dev --bin rubix-supervisor-fixture --release --locked && cargo test -p rubix-supervisor --locked --release --test signals --test owned_signals --no-run -j 2 && mkdir /out && cp target/release/rubix-supervisor-fixture /out/fixture && for suite in signals owned_signals; do count=0; for executable in target/release/deps/$suite-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/$suite.test; count=$((count + 1)); fi; done; test "$count" -eq 1 && test -x /out/$suite.test || exit 1; done
ARG QUALIFICATION_NONCE
RUN test -n "$QUALIFICATION_NONCE" && echo "RUBIX_BUILD_BEGIN $QUALIFICATION_NONCE" && sha256sum /out/signals.test /out/owned_signals.test /out/fixture && echo "RUBIX_BUILD_END $QUALIFICATION_NONCE"
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COPY --from=build /out/ /
COPY --chmod=755 tools/supervisor-signals/kill.sh /usr/local/bin/kill
USER 65532:65532
WORKDIR /tmp
ENTRYPOINT []
