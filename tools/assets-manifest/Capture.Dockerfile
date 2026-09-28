FROM golang:1.26.2-bookworm@sha256:47ce5636e9936b2c5cbf708925578ef386b4f8872aec74a67bd13a627d242b19 AS producer
ENV GOTOOLCHAIN=local CGO_ENABLED=0 GOFLAGS=-mod=readonly
WORKDIR /source
COPY tools/assets-manifest/go.mod tools/assets-manifest/go.sum ./
RUN mkdir /out && cp go.mod go.sum /out/ && go mod download all && go mod verify && cmp go.mod /out/go.mod && cmp go.sum /out/go.sum
COPY tools/assets-manifest/main.go tools/assets-manifest/main_test.go tools/assets-manifest/inputs.json ./
RUN test "$(go version)" = "go version go1.26.2 linux/arm64" && go test -timeout 20s . && go list -m -json all > /out/modules-actual.json && go build -trimpath -buildvcs=false -o /out/producer . && cmp go.mod /out/go.mod && cmp go.sum /out/go.sum
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /source
COPY . .
COPY --from=producer /out/ /producer-build/
RUN cargo build -p rubix-dev --bin rubix-asset-fixture --release --locked && target/release/rubix-asset-fixture manifest-graph /producer-build/modules-actual.json tools/assets-manifest/modules.json
RUN set -eu; cargo test -p rubix-assets --release --locked --test manifest_fixture --no-run && mkdir /out && cp target/release/rubix-asset-fixture /out/fixture && cp /producer-build/producer /out/producer && count=0; for executable in target/release/deps/manifest_fixture-*; do if [ -f "$executable" ] && [ -x "$executable" ]; then cp "$executable" /out/manifest-tests; count=$((count + 1)); fi; done; test "$count" -eq 1 && test -x /out/manifest-tests
ARG QUALIFICATION_NONCE
RUN test -n "$QUALIFICATION_NONCE" && echo "RUBIX_BUILD_BEGIN $QUALIFICATION_NONCE" && sha256sum /out/producer /out/manifest-tests /out/fixture && echo "RUBIX_BUILD_END $QUALIFICATION_NONCE"
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COPY --from=build /out/producer /producer
COPY --from=build /out/manifest-tests /manifest-tests
COPY --from=build /out/fixture /fixture
COPY manifest-inputs /inputs
USER 65532:65532
ENTRYPOINT []
