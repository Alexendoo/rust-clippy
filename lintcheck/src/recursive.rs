//! In `--recursive` mode we set the `lintcheck` binary as the `RUSTC_WRAPPER` of `cargo check`,
//! this allows [`crate::driver`] to be run for every dependency. The driver connects to
//! [`LintcheckServer`] to ask if it should be skipped, and if not sends the stderr of running
//! clippy on the crate to the server TODO

use crate::driver::DriverMode;
use crate::output::ClippyWarning;

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use cargo_metadata::diagnostic::Diagnostic;
use crossbeam_channel::{Receiver, Sender};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct DriverInfo {
    pub package_name: String,
    /// `CARGO_CRATE_NAME` - may be the name of a binary or the build script rather than the package
    /// name
    pub crate_name: String,
    pub version: String,
}

pub(crate) fn serialize_line<T, W>(value: &T, writer: &mut W)
where
    T: Serialize,
    W: Write,
{
    let mut buf = serde_json::to_vec(&value).expect("failed to serialize");
    buf.push(b'\n');
    writer.write_all(&buf).expect("write_all failed");
}

pub(crate) fn deserialize_line<T, R>(reader: &mut R) -> T
where
    T: DeserializeOwned,
    R: BufRead,
{
    let mut string = String::new();
    reader.read_line(&mut string).expect("read_line failed");
    serde_json::from_str(&string).expect("failed to deserialize")
}

fn process_stream(stream: TcpStream, sender: &Sender<ClippyWarning>, seen: &Mutex<HashSet<DriverInfo>>, perf: bool) {
    let mut stream = BufReader::new(stream);

    let driver: DriverInfo = deserialize_line(&mut stream);

    let mode = if true {
        if perf { DriverMode::Perf } else { DriverMode::Clippy }
    } else {
        DriverMode::Rustc
    };

    serialize_line(&mode, stream.get_mut());

    let mut stderr = String::new();
    stream.read_to_string(&mut stderr).unwrap();

    // It's 99% likely that dependencies compiled with recursive mode are on crates.io
    // and therefore on docs.rs. This links to the sources directly, do avoid invalid
    // links due to remapped paths. See rust-lang/docs.rs#2551 for more details.
    let base_url = format!(
        "https://docs.rs/crate/{}/{}/source/src/{{file}}#{{line}}",
        driver.package_name, driver.version
    );
    let messages = stderr
        .lines()
        .filter_map(|json_msg| serde_json::from_str::<Diagnostic>(json_msg).ok())
        .filter_map(|diag| ClippyWarning::new(diag, &base_url, &driver.package_name));

    for message in messages {
        sender.send(message).unwrap();
    }
}

pub(crate) struct LintcheckServer {
    pub local_addr: SocketAddr,
    receiver: Receiver<ClippyWarning>,
    sender: Arc<Sender<ClippyWarning>>,
}

impl LintcheckServer {
    pub fn spawn(perf: bool) -> Self {
        let listener = TcpListener::bind("localhost:0").unwrap();
        let local_addr = listener.local_addr().unwrap();

        let (sender, receiver) = crossbeam_channel::unbounded::<ClippyWarning>();
        let sender = Arc::new(sender);
        // The spawned threads hold a `Weak<Sender>` so that they don't keep the channel connected
        // indefinitely
        let sender_weak = Arc::downgrade(&sender);

        // Ignore dependencies multiple times, e.g. for when it's both checked and compiled for a
        // build dependency
        let seen = Mutex::default();

        thread::spawn(move || {
            thread::scope(|s| {
                s.spawn(|| {
                    while let Ok((stream, _)) = listener.accept() {
                        let sender = sender_weak.upgrade().expect("received connection after server closed");
                        let seen = &seen;
                        s.spawn(move || process_stream(stream, &sender, seen, perf));
                    }
                });
            });
        });

        Self {
            local_addr,
            receiver,
            sender,
        }
    }

    pub fn warnings(self) -> Vec<ClippyWarning> {
        // causes the channel to become disconnected so that the receiver iterator ends
        drop(self.sender);

        self.receiver.into_iter().collect()
    }
}
