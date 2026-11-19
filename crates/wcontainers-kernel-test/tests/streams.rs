//! Standard input, output, error and the terminal on the kernel's stream ends, compared with
//! `docker run -i`, `-t` and `-it` on real ARM64 Linux.

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use wcontainers_kernel::reason;
use wcontainers_kernel_test::driver::Run;
use wcontainers_kernel_test::reference::{ROOT_DIRECTORY, in_guest_directory};
use wcontainers_kernel_test::streams::{INPUT, Step, StreamClient};
use wcontainers_kernel_test::{assert_same_streams, guest_directory_entry, image_command};

fn out() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest")
}

/// Lines of words drawn from `seed`, `bytes` long or a little more.
fn text(seed: u64, bytes: usize) -> Vec<u8> {
    const WORDS: &[&str] = &["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta"];
    let mut random = StdRng::seed_from_u64(seed);
    let mut text = Vec::new();
    while text.len() < bytes {
        let word = WORDS[random.random_range(0..WORDS.len())];
        let number: u16 = random.random();
        text.extend_from_slice(format!("{word} {number}\n").as_bytes());
    }
    text
}

fn input(script: Vec<Step>) -> StreamClient {
    StreamClient {
        input: true,
        script,
        ..Default::default()
    }
}

fn send_all(bytes: Vec<u8>) -> Vec<Step> {
    vec![Step::Send(bytes), Step::CloseInput]
}

fn expect(text: &str) -> Step {
    Step::Expect(text.as_bytes().to_vec())
}

fn send(text: &str) -> Step {
    Step::Send(text.as_bytes().to_vec())
}

/// Checks every run gave no warning but those `allowed`.
fn only_warnings(runs: &[Run], allowed: &[&str]) {
    for run in runs {
        assert!(
            run.warnings
                .iter()
                .all(|warned| allowed.contains(&warned.reason.as_str())),
            "{:?}",
            run.warnings
        );
    }
}

/// The pipe size the program reported, which bounds the input credit.
fn pipe_size(run: &Run) -> u64 {
    let text = String::from_utf8_lossy(&run.outcome.stdout);
    let size = text
        .split_whitespace()
        .skip_while(|word| *word != "size")
        .nth(1)
        .expect("the program reports its pipe's size");
    size.parse().expect("a size")
}

#[test]
fn a_pipeline_reads_all_of_its_input() {
    let client = input(send_all(text(1, 200_000)));
    let runs = assert_same_streams(
        &out(),
        None,
        &[
            "sh",
            "-c",
            "sort | uniq -c | sort -k1,1nr -k2 | head -5; echo done",
        ],
        &client,
    );
    only_warnings(&runs, &[]);
}

#[test]
fn input_reaches_a_container_still_starting_on_the_backend() {
    // With the root on the backend the start waits for its answers; the client's credit and
    // input come meanwhile.
    let client = input(send_all(text(8, 20_000)));
    wcontainers_kernel_test::assert_same_streams_on(
        &out(),
        None,
        &["sh", "-c", "wc -l; echo done >&2"],
        &client,
        Some(ROOT_DIRECTORY),
    );
}

#[test]
fn end_of_input_is_end_of_file() {
    let client = input(send_all(text(2, 3000)));
    let runs = assert_same_streams(&out(), Some("stdin"), &["all", "100", "0"], &client);
    only_warnings(&runs, &[]);
    // No input at all, then end of file.
    let client = input(vec![Step::CloseInput]);
    assert_same_streams(&out(), Some("stdin"), &["all", "100", "0"], &client);
}

#[test]
fn large_input_to_a_slow_reader_is_held_to_the_credit() {
    let client = input(send_all(text(3, 1 << 20)));
    let runs = assert_same_streams(
        &out(),
        Some("stdin"),
        &["all", &(1 << 9).to_string(), "32"],
        &client,
    );
    for run in &runs {
        let size = pipe_size(run);
        assert!(
            run.most_input_credit <= size,
            "the kernel promised {} with a pipe of {size}",
            run.most_input_credit
        );
        assert!(run.most_input_credit > 0);
    }
}

#[test]
fn a_reader_that_closes_its_input_early() {
    let client = input(send_all(text(4, 1 << 20)));
    let runs = assert_same_streams(&out(), Some("stdin"), &["early", "100"], &client);
    for run in runs {
        assert!(run.input_closed, "the kernel closed the input end");
    }
}

#[test]
fn input_arrives_while_the_reader_waits() {
    for how in ["read", "poll", "select", "epoll"] {
        let client = input(vec![
            expect("waiting 1"),
            send("one\n"),
            expect("waiting 2"),
            send("two two\n"),
            expect("waiting 3"),
            Step::CloseInput,
        ]);
        assert_same_streams(&out(), Some("stdin"), &["wait", how], &client);
    }
}

#[test]
fn children_inherit_the_standard_descriptors() {
    // Each reader takes exactly what it shows (the shell's `read` and `dd` a byte at a time), so
    // what each sees does not depend on how the input was cut into pieces.
    let client = input(send_all(text(5, 20_000)));
    assert_same_streams(
        &out(),
        None,
        &[
            "sh",
            "-c",
            "read first; echo \"first: $first\"; dd bs=1 count=10 2>/dev/null | od -c; cat | wc -c; \
             echo error >&2",
        ],
        &client,
    );
}

#[test]
fn a_program_writing_its_own_input_pipe() {
    // The input pipe's space is the writer inside's too: credit given client does not hold it.
    let client = input(vec![expect("got"), Step::CloseInput]);
    assert_same_streams(
        &out(),
        None,
        &[
            "sh",
            "-c",
            "echo hi > /proc/self/fd/0; read x; echo got $x; echo more > /proc/self/fd/0; cat",
        ],
        &client,
    );
}

#[test]
fn a_terminal_closed_before_the_start_finished() {
    // With the root on the backend the start waits; the client lets go of the terminal before
    // it is made, and the slave is hung up as when the master closes, its node gone from devpts
    // (no reference: Docker's client cannot go before the container starts).
    std::fs::create_dir_all(out()).expect("creating the output directory");
    let client = StreamClient {
        input: true,
        terminal: Some((COLUMNS, ROWS)),
        script: vec![Step::CloseInput],
        ..Default::default()
    };
    let shell = image_command("sh");
    let command = [
        shell.as_str(),
        "-c",
        "read x; [ -e /dev/pts/0 ] && exit 8; exit 7",
    ];
    for run in run_ours(&out(), &command, client, Some(ROOT_DIRECTORY)) {
        assert_eq!(run.outcome.exit_code, 7, "{:?}", run.outcome);
        assert!(run.broken.is_empty(), "{:?}", run.broken);
    }
}

#[test]
fn the_standard_descriptors_redirected_away() {
    let client = input(send_all(text(6, 5 << 10)));
    assert_same_streams(
        &out(),
        None,
        &[
            "sh",
            "-c",
            "exec 3<&0 </dev/null; cat; echo [; cat <&3 | wc -c; exec 1>&2; echo to error; \
             ls -l /proc/self/fd/0 | sed 's/.*-> //'",
        ],
        &client,
    );
}

#[test]
fn large_output_to_a_slow_client() {
    for mode in ["block", "nonblock"] {
        let client = StreamClient {
            output_pace: Some(1 << 16),
            ..Default::default()
        };
        let runs = assert_same_streams(
            &out(),
            Some("stdout"),
            &["much", &(8 << 20).to_string(), mode],
            &client,
        );
        only_warnings(&runs, &[]);
        // Nonblocking, the writer met a full pipe (here and in the reference, compared above).
        let blocked = String::from_utf8_lossy(&runs[0].outcome.stderr).contains("would block yes");
        assert_eq!(blocked, mode == "nonblock");
    }
}

#[test]
fn output_after_the_client_closed() {
    let program = wcontainers_kernel_test::guest::build("stdout", &[], &out());
    let directory = program.parent().expect("a directory");
    for signal in ["default", "ignore"] {
        // As the first process, which a default SIGPIPE never kills, and as a shell's child,
        // which it does.
        for child in [false, true] {
            let command = |whose: &str| {
                let writer = format!("{} closed {whose} {signal}", in_guest_directory("stdout"));
                match child {
                    true => vec![
                        image_command("sh"),
                        "-c".to_string(),
                        format!("{writer}; echo status $? >&2"),
                    ],
                    false => writer.split(' ').map(str::to_string).collect(),
                }
            };
            // The reference shows a closed reader inside the container (see `run_with_client`).
            let theirs = command("self");
            let theirs: Vec<&str> = theirs.iter().map(String::as_str).collect();
            let reference = wcontainers_kernel_test::reference::run_with_client(
                directory,
                &theirs,
                &StreamClient::default(),
            );
            let ours = command("client");
            let ours: Vec<&str> = ours.iter().map(String::as_str).collect();
            let client = StreamClient {
                script: vec![expect("ready"), Step::CloseOutput],
                ..Default::default()
            };
            for run in run_ours(directory, &ours, client, None) {
                assert_eq!(run.outcome, reference, "{ours:?}");
                assert!(run.broken.is_empty(), "{:?}", run.broken);
            }
        }
    }
}

/// Our runs of `command` (its first word a path) with the guest programs of `directory` at
/// their place and the client given, under every schedule seed.
fn run_ours(
    directory: &Path,
    command: &[&str],
    client: StreamClient,
    backend: Option<&str>,
) -> Vec<Run> {
    let mut files = wcontainers_kernel_test::image::reference_image().to_vec();
    files.extend(wcontainers_kernel_test::image::host_directory(
        directory,
        guest_directory_entry(),
    ));
    let container = wcontainers_kernel_test::driver::Container {
        files: &files,
        path: command[0],
        arguments: command,
        environment: wcontainers_kernel_test::reference::ENVIRONMENT,
        working_directory: ROOT_DIRECTORY,
        backend,
        cached: false,
        published: &[],
        address_space: wcontainers_kernel_test::standin::SPACE_32_BIT,
    };
    wcontainers_kernel_test::schedule_seeds()
        .map(|seed| {
            wcontainers_kernel_test::driver::run_with_client(
                &container,
                client.clone(),
                seed,
                Default::default(),
            )
            .expect("an outcome on our kernel")
        })
        .collect()
}

/// Our runs of a guest program with `arguments` and the client given.
fn run_guest(name: &str, arguments: &[&str], client: StreamClient) -> Vec<Run> {
    let program = wcontainers_kernel_test::guest::build(name, &[], &out());
    let path = in_guest_directory(name);
    let mut command = vec![path.as_str()];
    command.extend_from_slice(arguments);
    run_ours(
        program.parent().expect("a directory"),
        &command,
        client,
        None,
    )
}

/// The size a terminal starts at when the test gives the client one.
const COLUMNS: u16 = 80;
const ROWS: u16 = 24;

fn terminal(input: bool, script: Vec<Step>) -> StreamClient {
    StreamClient {
        input,
        terminal: Some((COLUMNS, ROWS)),
        script,
        ..Default::default()
    }
}

#[test]
fn a_terminal_without_input() {
    // `docker run -t` with no terminal at the client has a terminal of no size.
    let client = StreamClient {
        terminal: Some((0, 0)),
        ..Default::default()
    };
    let runs = assert_same_streams(&out(), Some("terminal"), &["info"], &client);
    only_warnings(&runs, &[reason::PROC_FIELDS_NOT_TRACKED]);
}

#[test]
fn a_terminal_is_resized() {
    let client = terminal(
        true,
        vec![
            expect("ready"),
            Step::Resize {
                columns: 100,
                rows: 30,
            },
            expect("size 100 30"),
        ],
    );
    assert_same_streams(&out(), Some("terminal"), &["resize", "100", "30"], &client);
}

#[test]
fn canonical_and_raw_input_with_echo() {
    let client = terminal(
        true,
        vec![
            expect("ready"),
            send("hello\r"),
            expect("read 6"),
            send("abc\x7fd\x15xy\r"),
            expect("read 3"),
            send("\x04"),
            expect("read 0"),
            expect("raw"),
            send("xyz"),
            expect("done"),
        ],
    );
    assert_same_streams(&out(), Some("terminal"), &["lines"], &client);
}

#[test]
fn interrupt_and_suspend_reach_the_foreground_group() {
    let client = terminal(
        true,
        vec![
            expect("ready"),
            send("\x03"),
            expect("interrupted"),
            send("\x1a"),
            expect("child"),
        ],
    );
    assert_same_streams(&out(), Some("terminal"), &["signals"], &client);
}

#[test]
fn a_shell_on_the_terminal() {
    let client = terminal(
        true,
        vec![
            expect("# "),
            // `start` is printed by the job itself, once the line editor has given back the
            // terminal's settings and the job is the foreground group (the echoed line shows
            // `st""art`), so the ^C cannot reach the editor's raw mode.
            send("sh -c 'echo st\"\"art; exec sleep 30'\r"),
            expect("start"),
            send("\x03"),
            expect("^C"),
            expect("# "),
            send("echo $((6*7))\r"),
            expect("42"),
            send("exit\r"),
        ],
    );
    assert_same_streams(&out(), None, &["sh"], &client);
}

#[test]
fn a_client_sending_past_its_credit_is_cut_off() {
    let sent = text(7, 1 << 20);
    let client = StreamClient {
        input: true,
        script: send_all(sent.clone()),
        excess: 100,
        ..Default::default()
    };
    for run in run_guest(
        "stdin",
        &["all", &wcontainers_abi::PAGE_SIZE.to_string(), "0"],
        client,
    ) {
        let past: u64 = run
            .counters
            .iter()
            .filter(|counter| counter.reason == reason::STREAM_PAST_CREDIT)
            .map(|counter| counter.count)
            .sum();
        assert!(run.sends_past_credit > 0, "the client went past its credit");
        assert_eq!(
            past, run.sends_past_credit,
            "every send past the credit counted"
        );
        let text = String::from_utf8_lossy(&run.outcome.stdout);
        let total: u64 = text
            .split_whitespace()
            .skip_while(|word| *word != "total")
            .nth(1)
            .and_then(|total| total.parse().ok())
            .expect("a total");
        assert_eq!(
            total, run.sent_within_credit,
            "exactly what the credit allowed came in"
        );
        assert!(total < sent.len() as u64);
        assert!(
            run.warnings
                .iter()
                .any(|warned| warned.reason == reason::STREAM_PAST_CREDIT)
        );
    }
}

#[test]
fn credit_on_the_input_end_is_counted() {
    let mut script = vec![Step::Credit {
        end: INPUT,
        bytes: 1 << 30,
    }];
    script.extend(send_all(b"in\n".to_vec()));
    let client = StreamClient {
        input: true,
        script,
        ..Default::default()
    };
    for run in run_guest(
        "stdin",
        &["all", &wcontainers_abi::PAGE_SIZE.to_string(), "0"],
        client,
    ) {
        let counted: Vec<_> = run
            .counters
            .iter()
            .filter(|counter| counter.reason == reason::EVENT_UNEXPECTED)
            .map(|counter| (counter.detail, counter.count))
            .collect();
        assert_eq!(counted, [(u64::from(INPUT.0), 1)], "{:?}", run.counters);
        assert!(run.broken.is_empty(), "{:?}", run.broken);
    }
}

#[test]
fn a_client_that_never_reads_holds_the_writer_to_the_pipe() {
    let client = StreamClient {
        output_pace: Some(0),
        ..Default::default()
    };
    for run in run_guest("stdout", &["fill"], client) {
        assert!(run.broken.is_empty(), "{:?}", run.broken);
        assert!(run.outcome.stdout.is_empty());
        let report = String::from_utf8_lossy(&run.outcome.stderr).into_owned();
        let words: Vec<&str> = report.split_whitespace().collect();
        assert_eq!(
            words[1], words[3],
            "the pipe took its size and no more: {report}"
        );
        assert!(
            report.contains("Resource temporarily unavailable"),
            "{report}"
        );
        assert!(!run.outputs_closed, "what the pipe holds is still there");
    }
}
