# The server binary is compiled on ubuntu-24.04 in CI, so the runtime base must
# provide a glibc at least as new as that runner's (bookworm ships 2.36 and the
# binary needs 2.38/2.39). Keep this pinned together with `runs-on` in docker.yml.
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY bin/remux-server /app/remux-server

EXPOSE 8099

VOLUME /data

ENTRYPOINT ["/app/remux-server"]
CMD ["--datadir", "/data", "--host", "0.0.0.0", "--port", "8099"]
