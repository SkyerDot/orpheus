# Runtime base: the upstream image already ships everything Orpheus needs at
# runtime — jellyfin-ffmpeg (plus Intel OpenCL tone mapping), yt-dlp with the
# bgutil POT provider, and the built assets for the admin dashboard
# (/app/dashboard), jellyfin-web (/app/jellyfin-web) and remux-web
# (/app/remux-web) — wired up through WEB_PATH/DASHBOARD_PATH/REMUX_WEB_PATH.
# Only the server binary is replaced, which is the whole point of this fork.
#
# Bump the tag deliberately: it must stay close to the upstream commit this
# fork is based on, or the bundled assets drift from the binary.
FROM ghcr.io/lostb1t/remux:0.41.1

WORKDIR /app

COPY bin/remux-server /app/remux-server
RUN chmod +x /app/remux-server

# PORT also drives the base image's HEALTHCHECK, which probes /health, so it has
# to match the port the CMD binds.
ENV PORT=8099

EXPOSE 8099

VOLUME /data

CMD ["/app/remux-server", "--datadir", "/data", "--host", "0.0.0.0", "--port", "8099"]
