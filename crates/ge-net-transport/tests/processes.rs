//! One host and two clients as separate operating-system processes over
//! real QUIC on the loopback interface.
//!
//! The host's port is assigned by the operating system. Every wait has a
//! deadline and every child is killed if the test fails. Nothing here
//! reaches a public host, a matchmaking service or a game image.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_ge-net-loopback");
const DEADLINE: Duration = Duration::from_secs(90);

/// A child process whose output is collected and which never outlives the
/// test, even when an assertion fails.
struct Process {
    child: Child,
    lines: Receiver<String>,
    output: Vec<String>,
}

impl Process {
    fn spawn(arguments: &[&str]) -> Self {
        let mut child = Command::new(BINARY)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the loopback binary starts");
        let stdout = child.stdout.take().expect("piped stdout");
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            lines,
            output: Vec::new(),
        }
    }

    /// The first line starting with `prefix`, within the deadline.
    fn wait_line(&mut self, prefix: &str) -> String {
        let deadline = Instant::now() + DEADLINE;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = self
                .lines
                .recv_timeout(remaining)
                .unwrap_or_else(|_| panic!("no `{prefix}` line before the deadline"));
            self.output.push(line.clone());
            if line.starts_with(prefix) {
                return line;
            }
        }
    }

    /// Wait for exit within the deadline and return the exit code.
    fn finish(&mut self) -> i32 {
        let deadline = Instant::now() + DEADLINE;
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("child status") {
                break status;
            }
            assert!(Instant::now() < deadline, "process did not exit in time");
            std::thread::sleep(Duration::from_millis(10));
        };
        // The reader thread ends at end of file.
        while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(5)) {
            self.output.push(line);
        }
        status.code().expect("exited normally, not by signal")
    }

    /// `(tick, digest)` pairs from lines starting with `prefix`.
    fn digests(&self, prefix: &str) -> Vec<(u64, String)> {
        self.output
            .iter()
            .filter_map(|line| line.strip_prefix(prefix))
            .map(|rest| {
                let (tick, digest) = rest.trim().split_once(' ').expect("tick and digest");
                (tick.parse().expect("tick"), digest.to_string())
            })
            .collect()
    }

    fn field(&self, prefix: &str, name: &str) -> String {
        let line = self
            .output
            .iter()
            .find(|line| line.starts_with(prefix))
            .unwrap_or_else(|| panic!("no `{prefix}` line"));
        line.split_whitespace()
            .find_map(|word| word.strip_prefix(&format!("{name}=")))
            .unwrap_or_else(|| panic!("no `{name}` in `{line}`"))
            .to_string()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start a host and return it with its port and fingerprint.
fn start_host(clients: &str, ticks: &str) -> (Process, String, String) {
    let mut host = Process::spawn(&["host", "--clients", clients, "--ticks", ticks]);
    let listen = host.wait_line("LISTEN ");
    let mut words = listen.split_whitespace().skip(1);
    let port = words.next().expect("port").to_string();
    let fingerprint = words.next().expect("fingerprint").to_string();
    assert_ne!(port, "0", "the operating system assigned a port");
    assert_eq!(fingerprint.len(), 64);
    (host, port, fingerprint)
}

#[test]
fn a_host_and_two_clients_run_a_session_as_separate_processes() {
    let (mut host, port, fingerprint) = start_host("2", "180");
    let client_arguments = ["client", "--port", &port, "--fingerprint", &fingerprint];
    let mut clients = [
        Process::spawn(&client_arguments),
        Process::spawn(&client_arguments),
    ];

    assert_eq!(host.finish(), 0, "host exit code");
    for client in &mut clients {
        assert_eq!(client.finish(), 0, "client exit code");
    }

    let published: BTreeMap<u64, String> = host.digests("DIGEST ").into_iter().collect();
    let last_tick = *published.keys().next_back().expect("the host ticked");
    assert!(last_tick >= 180, "ran {last_tick} ticks");
    assert_eq!(host.field("HOST_DONE", "duplicates"), "0");
    assert_eq!(host.field("HOST_DONE", "rejected"), "0");
    assert_eq!(host.field("HOST_DONE", "shutdown_closes"), "2");
    let executed: u64 = host.field("HOST_DONE", "executed").parse().unwrap();

    let mut submitted = 0;
    for (index, client) in clients.iter().enumerate() {
        let applied = client.digests("APPLIED ");
        assert!(
            applied.len() >= 150,
            "client {index} applied only {} snapshots",
            applied.len()
        );
        // Every state a client applied is exactly what the host published
        // for that tick, and ticks only move forward.
        for pair in applied.windows(2) {
            assert!(pair[0].0 < pair[1].0, "client {index} went back in time");
        }
        for (tick, digest) in &applied {
            assert_eq!(
                published.get(tick),
                Some(digest),
                "client {index} applied a different state at tick {tick}"
            );
        }
        let newest = applied.last().unwrap().0;
        assert!(
            last_tick - newest <= 30,
            "client {index} ended {} ticks behind",
            last_tick - newest
        );
        // The host's orderly shutdown reached the client as a typed reason.
        assert_eq!(client.field("CLIENT_DONE", "reason"), "RemoteClosed(2)");
        let commands: u64 = client.field("CLIENT_DONE", "commands").parse().unwrap();
        assert!(
            commands >= 100,
            "client {index} submitted {commands} commands"
        );
        submitted += commands;
    }
    assert!(executed >= 200, "the host executed {executed} commands");
    assert!(executed <= submitted, "nothing ran that was not submitted");
}

#[test]
fn a_client_process_with_the_wrong_fingerprint_is_refused() {
    let (mut host, port, fingerprint) = start_host("1", "30");
    // Flip one digit: a well-formed pin for some other identity.
    let mut wrong = fingerprint.clone().into_bytes();
    wrong[0] = if wrong[0] == b'0' { b'1' } else { b'0' };
    let wrong = String::from_utf8(wrong).unwrap();

    let mut impostor_victim = Process::spawn(&["client", "--port", &port, "--fingerprint", &wrong]);
    assert_eq!(impostor_victim.finish(), 4, "the handshake must fail");
    assert_eq!(
        impostor_victim.field("CLIENT_DONE", "reason"),
        "Transport(AuthenticationFailed)"
    );
    assert_eq!(impostor_victim.field("CLIENT_DONE", "snapshots"), "0");

    // The host is unaffected and serves a client that pins it correctly.
    let mut client = Process::spawn(&["client", "--port", &port, "--fingerprint", &fingerprint]);
    assert_eq!(host.finish(), 0);
    assert_eq!(client.finish(), 0);
    assert_eq!(client.field("CLIENT_DONE", "reason"), "RemoteClosed(2)");
    assert_eq!(host.field("HOST_DONE", "shutdown_closes"), "1");
}

#[test]
fn the_binary_rejects_bad_usage() {
    for arguments in [&[][..], &["host"][..], &["client", "--port", "x"][..]] {
        let mut process = Process::spawn(arguments);
        assert_eq!(process.finish(), 2);
    }
}
