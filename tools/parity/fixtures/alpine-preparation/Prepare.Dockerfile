FROM golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd
WORKDIR /src
RUN curl --fail --location --retry 3 --output /tmp/source.tar.gz https://codeload.github.com/portainer/kubesolo/tar.gz/2ef1c4787989f11f868f81bb84ae2afd4a49a81d \
 && echo '9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec  /tmp/source.tar.gz' | sha256sum --check \
 && tar -xzf /tmp/source.tar.gz --strip-components=4 kubesolo-2ef1c4787989f11f868f81bb84ae2afd4a49a81d/internal/cli/preflight/preflight.go \
 && tar -xOf /tmp/source.tar.gz kubesolo-2ef1c4787989f11f868f81bb84ae2afd4a49a81d/go.sum > go.sum
COPY go.mod baseline_test.go ./
ENV CGO_ENABLED=0 GOTOOLCHAIN=local GOMAXPROCS=2
RUN sha256sum preflight.go > /source.sha256 && go test -mod=readonly -c -trimpath -o /preflight.test . && sha256sum /preflight.test >> /source.sha256
WORKDIR /tmp
ENTRYPOINT []

FROM alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
COPY --from=0 /preflight.test /preflight.test
COPY --from=0 /source.sha256 /source.sha256
COPY APKINDEX.tar.gz /repo/aarch64/APKINDEX.tar.gz
RUN printf "/repo\n" > /repositories
ENTRYPOINT []
