FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
RUN cargo test -p rubix-kube --test host_preflight --test iptables_probe --locked --no-run && cargo build -p rubix-kube --example assess_host --locked && mkdir /out && for suite in host_preflight iptables_probe; do for executable in target/debug/deps/$suite-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/$suite; fi; done; done && cp target/debug/examples/assess_host /out/assess_host && test -x /out/iptables_probe && test -x /out/host_preflight
FROM python:3.13.7-slim-bookworm@sha256:adafcc17694d715c905b4c7bebd96907a1fd5cf183395f0ebc4d3428bd22d92d
COPY --from=build /out/ /out/
COPY tools/node-assessment/iptables.py /iptables.py
COPY tools/node-assessment/consumer.py /consumer.py
COPY tools/supervisor-process/namespace_inventory.py /namespace_inventory.py
# Keep Docker's /sbin/docker-init mount outside the private /usr/sbin fixture tmpfs.
# This synthetic image layout changes no production lookup path or host filesystem.
RUN test -L /sbin && rm /sbin && mkdir /sbin
USER 65532:65532
ENTRYPOINT []
