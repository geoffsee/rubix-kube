FROM golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd
WORKDIR /source
RUN curl --fail --location --retry 3 --output /tmp/source.tar.gz https://codeload.github.com/portainer/kubesolo/tar.gz/2ef1c4787989f11f868f81bb84ae2afd4a49a81d \
 && echo '9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec  /tmp/source.tar.gz' | sha256sum --check \
 && tar -xzf /tmp/source.tar.gz --strip-components=1
COPY extract.go /extract.go
ENV CGO_ENABLED=0 GOTOOLCHAIN=local GOMAXPROCS=2
RUN go run /extract.go /source /oracle \
 && mkdir -p /oracle/internal/cli/ui && cp internal/cli/ui/ui.go /oracle/internal/cli/ui/ \
 && cp go.sum /oracle/ \
 && sha256sum internal/cli/cmd_check.go internal/cli/root.go internal/cli/helpers.go internal/cli/config/config.go internal/cli/cmd_version.go internal/cli/ui/ui.go > /source.sha256
WORKDIR /oracle
COPY go.mod ./
COPY capture_test.go cli/capture_test.go
COPY cases.json /cases.json
RUN go test -mod=readonly -c -trimpath -o /capture.test ./cli && sha256sum /capture.test >> /source.sha256
USER 65532:65532
WORKDIR /tmp
ENTRYPOINT []
