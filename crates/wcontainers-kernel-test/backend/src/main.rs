//! The first file backend for tests: fuse-backend-rs's passthrough file system over a directory.
//!
//! `wc-backend listen <directory>` serves FUSE messages over one TCP connection taken at [`PORT`],
//! each reply written back on it, for our kernel's backend hook: a byte is sent first to say it is
//! being served. `wc-backend mount <directory> <mount point>
//! <command>...` serves the same file system through Linux's own FUSE at the mount point, runs the
//! command there without the power to mount (as in a default container) and ends with its status:
//! the reference. Files are opened for direct reads and writes, unless `--cached` comes first: then
//! they are opened for the client's page cache, which shared mappings need.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fuse_backend_rs::api::server::Server;
use fuse_backend_rs::passthrough::{CachePolicy, Config, PassthroughFs};
use fuse_backend_rs::transport::{FuseBuf, FuseDevWriter, FuseSession, Reader};

mod announcing;
use announcing::Announcing;

/// The largest message either side sends: fuse-backend-rs's largest buffer and a header.
const LARGEST: usize = (1 << 20) + 4096;

/// The port `listen` takes its connection at.
const PORT: u16 = 9000;

fn main() {
    let mut arguments: Vec<String> = std::env::args().collect();
    let cached = arguments.get(1).is_some_and(|first| first == "--cached");
    if cached {
        arguments.remove(1);
    }
    let directory = arguments.get(2).expect("a directory to serve");
    // No name or attribute is kept for the client: each is asked for again.
    let config = Config {
        root_dir: directory.clone(),
        do_import: true,
        xattr: true,
        cache_policy: if cached {
            CachePolicy::Auto
        } else {
            CachePolicy::Never
        },
        entry_timeout: Duration::ZERO,
        attr_timeout: Duration::ZERO,
        ..Default::default()
    };
    let fs = PassthroughFs::<()>::new(config).expect("the passthrough file system");
    fs.import().expect("the directory served");
    let server = Arc::new(Server::new(Arc::new(Announcing(fs))));
    match arguments.get(1).map(String::as_str) {
        Some("listen") => listen(&server),
        Some("mount") => mount(server, Path::new(&arguments[3]), &arguments[4..]),
        _ => panic!(
            "wc-backend [--cached] listen <directory> | mount <directory> <mount point> <command>..."
        ),
    }
}

/// Takes one connection at [`PORT`], says it is served, then serves its messages, each reply
/// written back, until it ends.
fn listen(server: &Server<Arc<Announcing>>) {
    let listener = std::net::TcpListener::bind(("0.0.0.0", PORT)).expect("listening");
    let (mut input, _) = listener.accept().expect("a connection");
    input.set_nodelay(true).expect("no delay");
    input.write_all(&[0]).expect("saying it is served");
    let fd = input.as_raw_fd();
    let mut request = vec![0u8; LARGEST];
    let mut reply = vec![0u8; LARGEST];
    loop {
        if input.read_exact(&mut request[..4]).is_err() {
            return;
        }
        let length = u32::from_le_bytes(request[..4].try_into().expect("four bytes")) as usize;
        assert!(
            (4..=LARGEST).contains(&length),
            "a request of {length} bytes"
        );
        input
            .read_exact(&mut request[4..length])
            .expect("a whole request");
        let reader = Reader::from_fuse_buffer(FuseBuf::new(&mut request[..length]))
            .expect("a reader over the request");
        let writer = FuseDevWriter::<()>::new(fd, &mut reply).expect("a writer to the connection");
        if let Err(error) = server.handle_message(reader, writer.into(), None, None) {
            eprintln!("wc-backend: {error:?}");
        }
    }
}

/// Serves through Linux's FUSE at `at` while `command` runs there, then ends with its status.
fn mount(server: Arc<Server<Arc<Announcing>>>, at: &Path, command: &[String]) {
    let mut session = FuseSession::new(at, "wc-backend", "", false).expect("a FUSE session");
    session.mount().expect("mounted");
    let mut channel = session.new_channel().expect("a FUSE channel");
    // The device (a link to it) was only for mounting: the command sees `/dev` as a default
    // container has it.
    std::fs::remove_file("/dev/fuse").expect("removing /dev/fuse");
    std::thread::spawn(move || {
        while let Ok(Some((reader, writer))) = channel.get_request() {
            if let Err(error) = server.handle_message(reader, writer.into(), None, None) {
                eprintln!("wc-backend: {error:?}");
            }
        }
    });
    let mut run = std::process::Command::new(&command[0]);
    run.args(&command[1..]).current_dir(at);
    // SAFETY: only sets the umask and drops capabilities between fork and exec.
    unsafe {
        run.pre_exec(|| {
            // The server works with no umask; the command starts with Docker's.
            libc::umask(0o022);
            use caps::{CapSet, Capability};
            for set in [
                CapSet::Bounding,
                CapSet::Effective,
                CapSet::Permitted,
                CapSet::Inheritable,
            ] {
                caps::drop(None, set, Capability::CAP_SYS_ADMIN)
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
            }
            Ok(())
        });
    }
    let status = run.status().expect("the command started");
    let code = match (
        status.code(),
        std::os::unix::process::ExitStatusExt::signal(&status),
    ) {
        (Some(code), _) => code,
        (None, Some(signal)) => 128 + signal,
        _ => 1,
    };
    let _ = session.umount();
    std::process::exit(code);
}
