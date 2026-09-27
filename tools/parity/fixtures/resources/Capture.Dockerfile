FROM golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd AS build
WORKDIR /src
RUN curl --fail --location --retry 3 --output /tmp/source.tar.gz https://codeload.github.com/portainer/kubesolo/tar.gz/2ef1c4787989f11f868f81bb84ae2afd4a49a81d \
    && echo '9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec  /tmp/source.tar.gz' | sha256sum --check \
    && tar --extract --gzip --file /tmp/source.tar.gz --strip-components=1
ENV CGO_ENABLED=1 GOTOOLCHAIN=local GOFLAGS=-mod=readonly
COPY resources/coredns_capture_test.go /src/pkg/components/coredns/rubix_capture_test.go
COPY resources/localpath_capture_test.go /src/pkg/components/localpath/rubix_capture_test.go
COPY resources/portainer_capture_test.go /src/pkg/components/portainer/rubix_capture_test.go
COPY resources/d2k_capture_test.go /src/pkg/components/d2k/rubix_capture_test.go
COPY pki/pki_capture_test.go /src/internal/core/pki/rubix_capture_test.go
RUN mkdir /out && for component in coredns localpath portainer d2k; do go test -c -p 2 -tags external_deps -trimpath -o /out/$component.test ./pkg/components/$component || exit 1; done \
    && go test -c -p 2 -tags external_deps -trimpath -o /out/pki.test ./internal/core/pki \
    && go version > /out/go-version.txt
FROM build AS capture
USER 65532:65532
WORKDIR /tmp
ENTRYPOINT []
