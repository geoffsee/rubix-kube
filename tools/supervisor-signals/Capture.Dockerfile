FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo test -p rubix-supervisor --locked --release --test signals --no-run -j 2 \
    && mkdir /out \
    && for binary in target/release/deps/signals-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" /out/signals.test; fi; done \
    && test -x /out/signals.test \
    && sha256sum /out/signals.test > /out/signals.sha256
USER 65532:65532
WORKDIR /tmp
ENTRYPOINT []
