FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
WORKDIR /source
COPY . .
RUN cargo clippy -p rubix-platform --all-targets --locked -- -D warnings \
 && cargo test -p rubix-platform --locked --release --no-run \
 && mkdir /out \
 && for binary in target/release/deps/rubix_platform-* target/release/deps/discovery-* target/release/deps/preflight-* target/release/deps/constrained-* target/release/deps/preflight_probe-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" /out/; fi; done \
 && sha256sum /out/* > /out/binaries.sha256
USER 65532:65532
WORKDIR /tmp
ENTRYPOINT []
CMD ["sh", "-c", "cat /out/binaries.sha256; for binary in /out/rubix_platform-* /out/discovery-* /out/preflight-* /out/constrained-* /out/preflight_probe-*; do \"$binary\" --nocapture || exit; done"]
