#!/bin/bash
# Build a STATICALLY linked nginx for RISC-V64 using Alpine
set -ex

docker run --rm --platform linux/riscv64 \
  -v "$(pwd)/rootfs:/output" \
  mirror.gcr.io/library/alpine:edge \
  sh -c '
    set -ex
    apk add --no-cache build-base pcre2-dev zlib-dev linux-headers

    # Download nginx source
    cd /tmp
    wget -q https://nginx.org/download/nginx-1.26.3.tar.gz
    tar xf nginx-1.26.3.tar.gz
    cd nginx-1.26.3

    # Configure for minimal static build
    CFLAGS="-static" LDFLAGS="-static" ./configure \
        --prefix=/etc/nginx \
        --sbin-path=/usr/sbin/nginx \
        --conf-path=/etc/nginx/nginx.conf \
        --pid-path=/var/run/nginx.pid \
        --error-log-path=/var/log/nginx/error.log \
        --http-log-path=/var/log/nginx/access.log \
        --with-poll_module \
        --without-http_gzip_module \
        --without-http_rewrite_module \
        --without-http_ssl_module \
        --without-http_fastcgi_module \
        --without-http_uwsgi_module \
        --without-http_scgi_module \
        --without-http_grpc_module \
        --without-http_memcached_module \
        --without-http_empty_gif_module \
        --without-http_browser_module \
        --without-http_upstream_hash_module \
        --without-http_upstream_ip_hash_module \
        --without-http_upstream_least_conn_module \
        --without-http_upstream_random_module \
        --without-http_upstream_keepalive_module \
        --without-http_upstream_zone_module \
        --without-http_auth_basic_module \
        --without-http_autoindex_module \
        --without-http_geo_module \
        --without-http_map_module \
        --without-http_split_clients_module \
        --without-http_referer_module \
        --without-http_proxy_module \
        --without-mail_pop3_module \
        --without-mail_imap_module \
        --without-mail_smtp_module \
        --without-stream \
        --with-cc-opt="-static" \
        --with-ld-opt="-static"

    make -j$(nproc)

    # Copy result
    mkdir -p /output/usr/sbin /output/etc/nginx /output/var/log/nginx /output/var/lib/nginx/tmp /output/var/run /output/tmp /output/proc /output/dev /output/sys
    cp objs/nginx /output/usr/sbin/nginx

    # Copy config files
    cp conf/mime.types /output/etc/nginx/

    # Create a simple nginx config
    cat > /output/etc/nginx/nginx.conf << "NGINX_CONF"
worker_processes 1;
daemon off;
master_process off;
error_log /var/log/nginx/error.log;
pid /var/run/nginx.pid;

events {
    worker_connections 64;
    use poll;
}

http {
    include mime.types;
    default_type application/octet-stream;
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
  '
