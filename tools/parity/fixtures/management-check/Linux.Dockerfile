FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN rustup target add aarch64-unknown-linux-musl \
 && cargo clippy -p rubixctl --all-targets --target aarch64-unknown-linux-musl --locked -- -D warnings \
 && cargo test -p rubixctl --locked --release --target aarch64-unknown-linux-musl --test check --no-run \
 && cargo build -p rubix-dev --bin rubix-platform-management --release --locked \
 && mkdir /out && cp target/aarch64-unknown-linux-musl/release/rubixctl /out/rubixctl \
 && for binary in target/aarch64-unknown-linux-musl/release/deps/check-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" /out/check-tests; fi; done \
 && test -x /out/check-tests && sha256sum /out/* > /out/binaries.sha256
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COPY --from=build /out/ /
COPY --from=build /out/rubixctl /source/target/aarch64-unknown-linux-musl/release/rubixctl
COPY --from=build /source/target/release/rubix-platform-management /fixture
ENV RUBIX_MANAGEMENT_DISPOSABLE=1
ENTRYPOINT []
CMD ["sh","-c","cat /binaries.sha256 && /check-tests --nocapture && /fixture management-runtime"]
