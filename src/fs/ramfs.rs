use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::string::ToString;
use alloc::format;
use spin::Mutex;
use lazy_static::lazy_static;

/// A simple in-memory file system
pub struct RamFs {
    files: BTreeMap<String, RamFile>,
}

pub struct RamFile {
    pub data: &'static [u8],
    pub is_dir: bool,
}

lazy_static! {
    pub static ref RAMFS: Mutex<RamFs> = Mutex::new(RamFs::new());
}

impl RamFs {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn add_file(&mut self, path: &str, data: &'static [u8]) {
        self.files.insert(String::from(path), RamFile { data, is_dir: false });
        // Also add parent directories
        let mut p = String::from(path);
        while let Some(idx) = p.rfind('/') {
            p.truncate(idx);
            if p.is_empty() { break; }
            if !self.files.contains_key(&p) {
                self.files.insert(p.clone(), RamFile { data: &[], is_dir: true });
            }
        }
    }

    pub fn get_file(&self, path: &str) -> Option<&RamFile> {
        self.files.get(path)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn files_with_prefix(&self, prefix: &str) -> bool {
        self.files.keys().any(|k| k.starts_with(prefix))
    }

    pub fn list_dir(&self, path: &str) -> Vec<String> {
        let prefix = if path == "/" { String::from("/") } else { format!("{}/", path) };
        let mut entries = Vec::new();
        for key in self.files.keys() {
            if key.starts_with(&prefix) {
                let rest = &key[prefix.len()..];
                if !rest.contains('/') && !rest.is_empty() {
                    entries.push(rest.to_string());
                }
            }
        }
        entries
    }
}

pub fn init_ramfs() {
    let mut fs = RAMFS.lock();

    // Add nginx binary
    fs.add_file("/usr/sbin/nginx", include_bytes!("../../rootfs/usr/sbin/nginx"));

    // Add dynamic linker
    fs.add_file("/lib/ld-musl-riscv64.so.1", include_bytes!("../../rootfs/lib/ld-musl-riscv64.so.1"));

    // Add shared libraries
    fs.add_file("/usr/lib/libpcre2-8.so.0", include_bytes!("../../rootfs/usr/lib/libpcre2-8.so.0"));
    fs.add_file("/usr/lib/libssl.so.3", include_bytes!("../../rootfs/usr/lib/libssl.so.3"));
    fs.add_file("/usr/lib/libcrypto.so.3", include_bytes!("../../rootfs/usr/lib/libcrypto.so.3"));
    fs.add_file("/usr/lib/libz.so.1", include_bytes!("../../rootfs/usr/lib/libz.so.1"));

    // Add nginx config
    fs.add_file("/etc/nginx/nginx.conf", include_bytes!("../../rootfs/etc/nginx/nginx.conf"));
    fs.add_file("/etc/nginx/mime.types", include_bytes!("../../rootfs/etc/nginx/mime.types"));

    // Add web content
    fs.add_file("/var/www/index.html", include_bytes!("../../rootfs/var/www/index.html"));

    // Add necessary system files
    fs.add_file("/etc/passwd", b"root:x:0:0:root:/root:/bin/sh\nnginx:x:100:101:nginx:/var/lib/nginx:/sbin/nologin\nnobody:x:65534:65534:nobody:/:/sbin/nologin\n");
    fs.add_file("/etc/group", b"root:x:0:\nnginx:x:101:\nnogroup:x:65534:\n");
    fs.add_file("/etc/localtime", b""); // empty timezone = UTC
    fs.add_file("/etc/ssl/openssl.cnf", b"# minimal openssl config\nopenssl_conf = openssl_init\n[openssl_init]\n");
    fs.add_file("/etc/hosts", b"127.0.0.1 localhost\n");
    fs.add_file("/etc/resolv.conf", b"nameserver 10.0.2.3\n");

    // Add necessary directories as empty files
    fs.add_file("/var/log/nginx/.keep", b"");
    fs.add_file("/var/run/.keep", b"");
    fs.add_file("/var/lib/nginx/tmp/.keep", b"");
    fs.add_file("/var/lib/nginx/tmp/client_body/.keep", b"");
    fs.add_file("/var/lib/nginx/tmp/proxy/.keep", b"");
    fs.add_file("/var/lib/nginx/tmp/fastcgi/.keep", b"");
    fs.add_file("/var/lib/nginx/tmp/uwsgi/.keep", b"");
    fs.add_file("/var/lib/nginx/tmp/scgi/.keep", b"");
    fs.add_file("/tmp/.keep", b"");

    // Add /dev/null as a marker
    fs.add_file("/dev/null", b"");

    let count = fs.files.len();
    println!("[FS] RamFS initialized with {} entries", count);
}
