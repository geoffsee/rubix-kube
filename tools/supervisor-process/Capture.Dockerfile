FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo test -p rubix-supervisor --test process_ownership --locked --no-run && mkdir /out && for executable in target/debug/deps/process_ownership-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/process-tests; fi; done && test -x /out/process-tests
FROM python:3.13.7-slim-bookworm@sha256:adafcc17694d715c905b4c7bebd96907a1fd5cf183395f0ebc4d3428bd22d92d
COPY --from=build /out/process-tests /process-tests
COPY tools/supervisor-process/fixture.py /fixture.py
COPY tools/supervisor-process/namespace_inventory.py /namespace_inventory.py
USER 65532:65532
ENTRYPOINT []
