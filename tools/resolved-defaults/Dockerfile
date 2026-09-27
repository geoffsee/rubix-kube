FROM golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd AS build
COPY go.tar.gz /tmp/go.tar.gz
RUN echo '211ffced9dcb9633a55eac6364816ec0ddd951389a740e88fa8b3337971bdda0  /tmp/go.tar.gz' | sha256sum -c - && mkdir /toolchain && tar -xzf /tmp/go.tar.gz -C /toolchain
ENV PATH=/toolchain/go/bin:$PATH GOTOOLCHAIN=local CGO_ENABLED=0 GOMAXPROCS=2 GOPROXY=off GOSUMDB=off
COPY source.tar.gz /tmp/source.tar.gz
ARG SOURCE_SHA256
RUN echo "${SOURCE_SHA256}  /tmp/source.tar.gz" | sha256sum -c - && mkdir /src && tar -xzf /tmp/source.tar.gz -C /src --strip-components=1
WORKDIR /src
COPY main.go /src/cmd/rubix-resolved/
RUN go version && mkdir /out && go build -mod=vendor -p 2 -trimpath -o /out/extract ./cmd/rubix-resolved && sha256sum go.mod go.sum go.work vendor/modules.txt > /out/modules.sha256
FROM scratch
COPY --from=build /out/ /out/
USER 65532:65532
ENTRYPOINT ["/out/extract"]
