FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo build -p rubix-dev --bin rubix-asset-fixture --release --locked && cargo test -p rubix-assets --release --locked --test decode --test decoded_elf --no-run && mkdir /out && cp target/release/rubix-asset-fixture /out/fixture && for suite in decode decoded_elf; do count=0; for executable in target/release/deps/$suite-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/$suite-tests; count=$((count + 1)); fi; done; test "$count" -eq 1 && test -x /out/$suite-tests || exit 1; done
ARG QUALIFICATION_NONCE
RUN test -n "$QUALIFICATION_NONCE" && echo "RUBIX_BUILD_BEGIN $QUALIFICATION_NONCE" && sha256sum /out/decode-tests /out/decoded_elf-tests /out/fixture && echo "RUBIX_BUILD_END $QUALIFICATION_NONCE"
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COPY --from=build /out/decode-tests /decode-tests
COPY --from=build /out/decoded_elf-tests /decoded_elf-tests
COPY --from=build /out/fixture /fixture
USER 65532:65532
ENTRYPOINT []
