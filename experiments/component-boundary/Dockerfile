FROM python:3.13.7-slim-bookworm@sha256:adafcc17694d715c905b4c7bebd96907a1fd5cf183395f0ebc4d3428bd22d92d
# The base includes OpenSSL libraries; the CLI generates disposable test credentials.
RUN apt-get update && apt-get install -y --no-install-recommends openssl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /experiment
COPY inputs.json fetch.py ./
RUN python fetch.py
COPY spike.py ./
RUN mkdir /evidence /state && chown -R 65532:65532 /evidence /state
USER 65532:65532
ENTRYPOINT ["python", "/experiment/spike.py"]
