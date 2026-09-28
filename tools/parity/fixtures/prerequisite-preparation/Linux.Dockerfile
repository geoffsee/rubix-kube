FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN rustup target add aarch64-unknown-linux-musl \
 && cargo clippy -p rubixctl --all-targets --target aarch64-unknown-linux-musl --locked -- -D warnings \
 && cargo test -p rubixctl --locked --release --target aarch64-unknown-linux-musl --lib --test check --test preparation --no-run \
 && mkdir /out && cp target/aarch64-unknown-linux-musl/release/rubixctl /out/rubixctl \
 && for kind in check preparation; do for binary in target/aarch64-unknown-linux-musl/release/deps/$kind-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" /out/$kind-tests; fi; done; done \
 && for binary in target/aarch64-unknown-linux-musl/release/deps/rubixctl-*; do if [ -f "$binary" ] && [ -x "$binary" ] && "$binary" --list 2>/dev/null | grep -q '^preparation::tests::'; then cp "$binary" /out/rubixctl-tests; fi; done \
 && test -x /out/rubixctl-tests \
 && rustc --edition=2024 --target aarch64-unknown-linux-musl -O tools/parity/fixtures/prerequisite-preparation/fixture-command.rs -o /out/fixture-command \
 && sha256sum /out/* > /out/binaries.sha256 \
 && cargo build -p rubix-dev --bin rubix-prerequisite-fixture --release --target aarch64-unknown-linux-musl --locked \
 && cp target/aarch64-unknown-linux-musl/release/rubix-prerequisite-fixture /out/rubix-prerequisite-fixture
FROM alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
COPY --from=build /out/ /
COPY --from=build /out/rubixctl /source/target/aarch64-unknown-linux-musl/release/rubixctl
ENTRYPOINT []
CMD ["sh","-c","cat /binaries.sha256 && /rubixctl-tests --include-ignored --nocapture && /check-tests --nocapture && /preparation-tests --nocapture && /rubix-prerequisite-fixture --run-disposable-fixture"]
