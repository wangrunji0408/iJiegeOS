#!/bin/bash
# Build a statically linked nginx for RISC-V64
set -ex

# Use Alpine Linux RISC-V64 to build nginx with musl (static)
docker run --rm --platform linux/riscv64 \
  -v "$(pwd)/rootfs:/output" \
  riscv64/alpine:edge \
  sh -c '
    apk add --no-cache nginx nginx-mod-http-headers-more

    # Copy nginx binary and dependencies
    mkdir -p /output/usr/sbin /output/etc/nginx /output/var/log/nginx /output/var/lib/nginx /output/var/run /output/usr/lib/nginx/modules /output/tmp /output/proc /output/dev /output/sys

    cp /usr/sbin/nginx /output/usr/sbin/
    cp -r /etc/nginx/* /output/etc/nginx/ 2>/dev/null || true

    # Copy shared libraries needed
    mkdir -p /output/lib /output/usr/lib
    cp /lib/ld-musl-riscv64*.so* /output/lib/ 2>/dev/null || true
    cp -a /lib/libc.musl-riscv64*.so* /output/lib/ 2>/dev/null || true
    for lib in /usr/lib/libpcre*.so* /usr/lib/libssl*.so* /usr/lib/libcrypto*.so* /usr/lib/libz*.so* /lib/libz*.so*; do
      [ -f "$lib" ] && cp -a "$lib" /output/usr/lib/ 2>/dev/null || true
    done
    # Copy all needed shared libs
    for f in $(ldd /usr/sbin/nginx 2>/dev/null | grep -o "/[^ ]*"); do
      dir=$(dirname "$f")
      mkdir -p "/output$dir"
      cp -a "$f" "/output$dir/" 2>/dev/null || true
    done

    # Create a simple nginx config
    cat > /output/etc/nginx/nginx.conf << "NGINX_CONF"
worker_processes 1;
daemon off;
master_process off;
error_log /dev/null;
pid /var/run/nginx.pid;

events {
    worker_connections 64;
    use epoll;
}

http {
    access_log off;
    sendfile off;

    server {
        listen 80;
        server_name localhost;

        location / {
            root /var/www;
            index index.html;
        }
    }
}
NGINX_CONF

    # Create a simple web page
    mkdir -p /output/var/www
    cat > /output/var/www/index.html << "HTML"
<!DOCTYPE html>
<html>
<head><title>JiegeOS</title></head>
<body>
<h1>Welcome to JiegeOS!</h1>
<p>Nginx is running on a custom RISC-V OS kernel written in Rust.</p>
</body>
</html>
HTML

    echo "Build complete!"
    ls -la /output/usr/sbin/nginx
    file /output/usr/sbin/nginx
    ldd /output/usr/sbin/nginx 2>/dev/null || echo "static or no ldd"
  '
