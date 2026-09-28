FROM golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd
WORKDIR /source
RUN curl --fail --location --retry 3 --output /tmp/source.tar.gz https://codeload.github.com/portainer/kubesolo/tar.gz/2ef1c4787989f11f868f81bb84ae2afd4a49a81d \
 && echo '9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec  /tmp/source.tar.gz' | sha256sum --check \
 && tar -xzf /tmp/source.tar.gz --strip-components=1
COPY extract.go /extract.go
ENV CGO_ENABLED=0 GOTOOLCHAIN=local GOMAXPROCS=2
RUN go run /extract.go /source /oracle \
 && mkdir -p /oracle/network /oracle/system \
 && cp internal/runtime/network/ipv6.go /oracle/network/source.go \
 && cp internal/system/modules.go /oracle/system/source.go \
 && cp go.sum /oracle/go.sum \
 && sha256sum internal/runtime/network/ipv6.go internal/system/modules.go pkg/kubernetes/kubeproxy/flags.go internal/core/embedded/host.go types/const.go > /source.sha256
WORKDIR /oracle
COPY go.mod ./
COPY network_test.go network/capture_test.go
COPY system_test.go system/capture_test.go
COPY proxy_test.go proxy/capture_test.go
COPY embedded_test.go embedded/capture_test.go
RUN for pkg in network system proxy embedded; do go test -mod=readonly -c -trimpath -o /$pkg.test ./$pkg || exit; sha256sum /$pkg.test >> /source.sha256; done
WORKDIR /tmp
ENTRYPOINT []
