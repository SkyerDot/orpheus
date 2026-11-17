FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY bin/remux-server /app/remux-server

ENV ORPHEUS_DATA_DIR=/data
ENV ORPHEUS_HOST=0.0.0.0
ENV ORPHEUS_PORT=8099

EXPOSE 8099

VOLUME /data

ENTRYPOINT ["/app/remux-server"]
CMD ["--datadir", "/data", "--host", "0.0.0.0", "--port", "8099"]
