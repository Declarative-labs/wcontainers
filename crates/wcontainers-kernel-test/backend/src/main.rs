//! The first file backend for tests: fuse-backend-rs's passthrough file system over a directory.
//!
//! `wc-backend pipe <directory>` serves FUSE messages read from standard input and writes each reply
//! to standard output, for our kernel's backend hook. `wc-backend mount <directory> <mount point>
//! <command>...` serves the same file system through Linux's own FUSE at the mount point, runs the
//! command there without the power to mount (as in a default container) and ends with its status:
//! the reference.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fuse_backend_rs::api::server::Server;
use fuse_backend_rs::passthrough::{CachePolicy, Config, PassthroughFs};
use fuse_backend_rs::transport::{FuseBuf, FuseDevWriter, FuseSession, Reader};

/// The largest message either side sends: fuse-backend-rs's largest buffer and a header.
const LARGEST: usize = (1 << 20) + 4096;

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let directory = arguments.get(2).expect("a directory to serve");
    // Nothing is kept for the client: every name and attribute is asked for again.
    let config = Config {
        root_dir: directory.clone(),
        do_import: true,
        xattr: true,
        cache_policy: CachePolicy::Never,
        entry_timeout: Duration::ZERO,
        attr_timeout: Duration::ZERO,
        ..Default::default()
    };
    let fs = PassthroughFs::<()>::new(config).expect("the passthrough file system");
    fs.import().expect("the directory served");
    let server = Arc::new(Server::new(Arc::new(fs)));
    match arguments.get(1).map(String::as_str) {
        Some("pipe") => pipe(&server),
        Some("mount") => mount(server, Path::new(&arguments[3]), &arguments[4..]),
        _ => panic!("wc-backend pipe <directory> | mount <directory> <mount point> <command>..."),
    }
}

/// Serves messages from standard input, each reply to standard output, until the input ends.
fn pipe(server: &Server<Arc<PassthroughFs>>) {
    let mut input = std::io::stdin().lock();
    let mut request = vec![0u8; LARGEST];
    let mut reply = vec![0u8; LARGEST];
    loop {
        if input.read_exact(&mut request[..4]).is_err() {
            return;
        }
        let length = u32::from_le_bytes(request[..4].try_into().expect("four bytes")) as usize;
        assert!((4..=LARGEST).contains(&length), "a request of {length} bytes");
        input
            .read_exact(&mut request[4..length])
            .expect("a whole request");
        let reader = Reader::from_fuse_buffer(FuseBuf::new(&mut request[..length]))
            .expect("a reader over the request");
        let writer = FuseDevWriter::<()>::new(1, &mut reply).expect("a writer to standard output");
        if let Err(error) = server.handle_message(reader, writer.into(), None, None) {
            eprintln!("wc-backend: {error:?}");
        }
    }
}

/// Serves through Linux's FUSE at `at` while `command` runs there, then ends with its status.
fn mount(server: Arc<Server<Arc<PassthroughFs>>>, at: &Path, command: &[String]) {
    let mut session = FuseSession::new(at, "wc-backend", "", false).expect("a FUSE session");
    session.mount().expect("mounted");
    let mut channel = session.new_channel().expect("a FUSE channel");
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
    let code = match (status.code(), std::os::unix::process::ExitStatusExt::signal(&status)) {
        (Some(code), _) => code,
        (None, Some(signal)) => 128 + signal,
        _ => 1,
    };
    let _ = session.umount();
    std::process::exit(code);
}
