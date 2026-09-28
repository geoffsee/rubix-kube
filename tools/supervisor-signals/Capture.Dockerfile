FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo test -p rubix-supervisor --locked --release --test signals --test owned_signals --no-run -j 2 \
    && mkdir /out \
    && for binary in target/release/deps/signals-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" /out/signals.test; fi; done \
    && test -x /out/signals.test \
    && sha256sum /out/signals.test > /out/signals.sha256
RUN for binary in target/release/deps/owned_signals-*; do if [ -f "$binary" ] && [ -x "$binary" ]; then cp "$binary" /out/owned_signals.test; fi; done && test -x /out/owned_signals.test && sha256sum /out/owned_signals.test > /out/owned_signals.sha256
FROM python:3.13.7-slim-bookworm@sha256:adafcc17694d715c905b4c7bebd96907a1fd5cf183395f0ebc4d3428bd22d92d
COPY --from=build /out /out
COPY tools/supervisor-process/fixture.py /fixture.py
COPY tools/supervisor-process/namespace_inventory.py /namespace_inventory.py
COPY --chmod=755 tools/supervisor-signals/kill.sh /usr/local/bin/kill
USER 65532:65532
WORKDIR /tmp
ENTRYPOINT []
