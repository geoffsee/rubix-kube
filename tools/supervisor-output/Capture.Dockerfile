FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo build -p rubix-dev --bin rubix-supervisor-fixture --release --locked && cargo test -p rubix-supervisor --test output --locked --no-run && mkdir /out && cp target/release/rubix-supervisor-fixture /out/fixture && count=0; for executable in target/debug/deps/output-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/output-tests; count=$((count + 1)); fi; done; test "$count" -eq 1 && test -x /out/output-tests
ARG QUALIFICATION_NONCE
RUN test -n "$QUALIFICATION_NONCE" && echo "RUBIX_BUILD_BEGIN $QUALIFICATION_NONCE" && sha256sum /out/output-tests /out/fixture && echo "RUBIX_BUILD_END $QUALIFICATION_NONCE"
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COPY --from=build /out/ /
USER 65532:65532
ENTRYPOINT []
