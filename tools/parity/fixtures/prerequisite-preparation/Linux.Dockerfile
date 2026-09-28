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
 && sha256sum /out/* > /out/binaries.sha256
FROM python:3.13.7-slim-bookworm@sha256:adafcc17694d715c905b4c7bebd96907a1fd5cf183395f0ebc4d3428bd22d92d
COPY --from=build /out/ /
COPY --from=build /out/rubixctl /source/target/aarch64-unknown-linux-musl/release/rubixctl
COPY tools/parity/fixtures/prerequisite-preparation/linux_fixture.py /linux_fixture.py
ENTRYPOINT []
CMD ["sh","-c","cat /binaries.sha256 && /rubixctl-tests --include-ignored --nocapture && /check-tests --nocapture && /preparation-tests --nocapture && python3 /linux_fixture.py"]
