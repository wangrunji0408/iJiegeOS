//! Initial RAM filesystem: embed the nginx payload (binary, libs, config) into
//! the kernel image at build time.
use crate::fs::Vfs;

macro_rules! blob {
    ($path:literal) => {{
        const B: &[u8] = include_bytes!(concat!("../../vendor/", $path));
        B
    }};
}

static LD_MUSL: &[u8] = blob!("musl/lib/ld-musl-riscv64.so.1");
static NGINX: &[u8] = blob!("nginx/usr/sbin/nginx");
static LIBCRYPTO: &[u8] = blob!("libcrypto3/usr/lib/libcrypto.so.3");
static LIBSSL: &[u8] = blob!("libssl3/usr/lib/libssl.so.3");
static LIBPCRE2: &[u8] = blob!("pcre2/usr/lib/libpcre2-8.so.0.15.0");
static LIBZ: &[u8] = blob!("zlib/usr/lib/libz.so.1.3.2");

static NGINX_CONF: &[u8] = b"\
daemon off;\n\
master_process off;\n\
worker_processes 1;\n\
error_log /dev/stderr warn;\n\
pid /run/nginx.pid;\n\
events { worker_connections 16; }\n\
http {\n\
    access_log off;\n\
    default_type application/octet-stream;\n\
    sendfile off;\n\
    keepalive_timeout 0;\n\
    server {\n\
        listen 80 default_server;\n\
        server_name _;\n\
        root /var/www;\n\
        index index.html;\n\
    }\n\
}\n";

static PASSWD: &[u8] = b"root:x:0:0:root:/root:/bin/sh\nnginx:x:100:101:nginx:/var/lib/nginx:/sbin/nologin\n";
static GROUP: &[u8] = b"root:x:0:\nnginx:x:101:\n";
static RESOLV_CONF: &[u8] = b"nameserver 8.8.8.8\n";
static HOSTS: &[u8] = b"127.0.0.1 localhost\n";

static INDEX_HTML: &[u8] = b"<!doctype html>\n<html><body><h1>Hello from iJiege Rust kernel + nginx</h1></body></html>\n";

static MIME_TYPES: &[u8] = b"\
types {\n\
    text/html html htm;\n\
    text/plain txt;\n\
    text/css css;\n\
    application/javascript js;\n\
    image/png png;\n\
}\n";

pub fn load(vfs: &Vfs) {
    vfs.insert_dir("/");
    vfs.insert_dir("/lib");
    vfs.insert_dir("/usr");
    vfs.insert_dir("/usr/lib");
    vfs.insert_dir("/usr/sbin");
    vfs.insert_dir("/etc");
    vfs.insert_dir("/etc/nginx");
    vfs.insert_dir("/var");
    vfs.insert_dir("/var/www");
    vfs.insert_dir("/var/log");
    vfs.insert_dir("/var/log/nginx");
    vfs.insert_dir("/var/lib");
    vfs.insert_dir("/var/lib/nginx");
    vfs.insert_dir("/var/lib/nginx/logs");
    vfs.insert_dir("/var/lib/nginx/tmp");
    vfs.insert_dir("/var/lib/nginx/tmp/client_body");
    vfs.insert_dir("/var/lib/nginx/tmp/fastcgi");
    vfs.insert_dir("/var/lib/nginx/tmp/proxy");
    vfs.insert_dir("/var/lib/nginx/tmp/scgi");
    vfs.insert_dir("/var/lib/nginx/tmp/uwsgi");
    vfs.insert_dir("/run");
    vfs.insert_dir("/dev");
    vfs.insert_dir("/tmp");
    vfs.insert_dir("/proc");
    vfs.insert_dir("/proc/self");

    vfs.insert_file("/lib/ld-musl-riscv64.so.1", LD_MUSL);
    vfs.insert_symlink("/lib/libc.musl-riscv64.so.1", "/lib/ld-musl-riscv64.so.1");
    vfs.insert_file("/usr/sbin/nginx", NGINX);
    vfs.insert_file("/usr/lib/libcrypto.so.3", LIBCRYPTO);
    vfs.insert_file("/usr/lib/libssl.so.3", LIBSSL);
    vfs.insert_file("/usr/lib/libpcre2-8.so.0", LIBPCRE2);
    vfs.insert_file("/usr/lib/libpcre2-8.so.0.15.0", LIBPCRE2);
    vfs.insert_file("/usr/lib/libz.so.1", LIBZ);
    vfs.insert_file("/usr/lib/libz.so.1.3.2", LIBZ);

    vfs.insert_file("/etc/nginx/nginx.conf", NGINX_CONF);
    vfs.insert_file("/etc/nginx/mime.types", MIME_TYPES);
    vfs.insert_file("/etc/passwd", PASSWD);
    vfs.insert_file("/etc/group", GROUP);
    vfs.insert_file("/etc/resolv.conf", RESOLV_CONF);
    vfs.insert_file("/etc/hosts", HOSTS);
    vfs.insert_file("/var/www/index.html", INDEX_HTML);
}
