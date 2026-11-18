//! Standard input, output, error and the terminal on the kernel's stream ends, compared with
//! `docker run -i`, `-t` and `-it` on real ARM64 Linux.

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use wcontainers_kernel::reason;
use wcontainers_kernel_test::assert_same_streams;
use wcontainers_kernel_test::driver::Run;
use wcontainers_kernel_test::streams::{Outside, Step};

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
        let number: u16 = random.random_range(0..1000);
        text.extend_from_slice(format!("{word} {number}\n").as_bytes());
    }
    text
}

fn input(script: Vec<Step>) -> Outside {
    Outside {
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
    let outside = input(send_all(text(1, 200_000)));
    let runs = assert_same_streams(
        &out(),
        None,
        &[
            "sh",
            "-c",
            "sort | uniq -c | sort -k1,1nr -k2 | head -5; echo done",
        ],
        &outside,
    );
    only_warnings(&runs, &[]);
}

#[test]
fn input_reaches_a_container_still_starting_on_the_backend() {
    // With the root on the backend the start waits for its answers; the outside's credit and
    // input come meanwhile.
    let outside = input(send_all(text(8, 20_000)));
    wcontainers_kernel_test::assert_same_streams_on(
        &out(),
        None,
        &["sh", "-c", "wc -l; echo done >&2"],
        &outside,
        Some("/"),
    );
}

#[test]
fn end_of_input_is_end_of_file() {
    let outside = input(send_all(text(2, 3000)));
    let runs = assert_same_streams(&out(), Some("stdin"), &["all", "100", "0"], &outside);
    only_warnings(&runs, &[]);
    // No input at all, then end of file.
    let outside = input(vec![Step::CloseInput]);
    assert_same_streams(&out(), Some("stdin"), &["all", "100", "0"], &outside);
}

#[test]
fn large_input_to_a_slow_reader_is_held_to_the_credit() {
    let outside = input(send_all(text(3, 1 << 20)));
    let runs = assert_same_streams(&out(), Some("stdin"), &["all", "512", "32"], &outside);
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
    let outside = input(send_all(text(4, 1 << 20)));
    let runs = assert_same_streams(&out(), Some("stdin"), &["early", "100"], &outside);
    for run in runs {
        assert!(run.input_closed, "the kernel closed the input end");
    }
}

#[test]
fn input_arrives_while_the_reader_waits() {
    for how in ["read", "poll", "select", "epoll"] {
        let outside = input(vec![
            expect("waiting 1"),
            send("one\n"),
            expect("waiting 2"),
            send("two two\n"),
            expect("waiting 3"),
            Step::CloseInput,
        ]);
        assert_same_streams(&out(), Some("stdin"), &["wait", how], &outside);
    }
}

#[test]
fn children_inherit_the_standard_descriptors() {
    let outside = input(send_all(text(5, 20_000)));
    assert_same_streams(
        &out(),
        None,
        &[
            "sh",
            "-c",
            "read first; echo \"first: $first\"; head -c 10 | od -c; cat | wc -c; echo error >&2",
        ],
        &outside,
    );
}

#[test]
fn the_standard_descriptors_redirected_away() {
    let outside = input(send_all(text(6, 5000)));
    assert_same_streams(
        &out(),
        None,
        &[
            "sh",
            "-c",
            "exec 3<&0 </dev/null; cat; echo [; cat <&3 | wc -c; exec 1>&2; echo to error; \
             ls -l /proc/self/fd/0 | sed 's/.*-> //'",
        ],
        &outside,
    );
}

#[test]
fn large_output_to_a_slow_outside() {
    for mode in ["block", "nonblock"] {
        let outside = Outside {
            output_pace: Some(1 << 16),
            ..Default::default()
        };
        let runs =
            assert_same_streams(&out(), Some("stdout"), &["much", "8388608", mode], &outside);
        only_warnings(&runs, &[]);
        // Nonblocking, the writer met a full pipe (here and in the reference, compared above).
        let blocked = String::from_utf8_lossy(&runs[0].outcome.stderr).contains("would block yes");
        assert_eq!(blocked, mode == "nonblock");
    }
}

#[test]
fn output_after_the_outside_closed() {
    for signal in ["default", "ignore"] {
        // The reference shows a closed reader inside the container (see `run_outside`).
        let theirs = Outside::default();
        let ours = Outside {
            script: vec![expect("ready"), Step::CloseOutput],
            ..Default::default()
        };
        let program = wcontainers_kernel_test::guest::build("stdout", &[], &out());
        let reference = wcontainers_kernel_test::reference::run_outside(
            program.parent().expect("a directory"),
            &["/wc/stdout", "closed", "self", signal],
            &theirs,
        );
        let runs = run_ours("stdout", &["closed", "outside", signal], ours);
        for run in runs {
            assert_eq!(run.outcome, reference);
            assert!(run.broken.is_empty(), "{:?}", run.broken);
        }
    }
}

/// Our runs of a guest program with the outside given, under every schedule seed.
fn run_ours(name: &str, arguments: &[&str], outside: Outside) -> Vec<Run> {
    let program = wcontainers_kernel_test::guest::build(name, &[], &out());
    let directory = program.parent().expect("a directory");
    let path = format!("/wc/{name}");
    let mut all = vec![path.as_str()];
    all.extend_from_slice(arguments);
    let mut files = wcontainers_kernel_test::image::reference_image().to_vec();
    files.extend(wcontainers_kernel_test::image::host_directory(
        directory, "wc",
    ));
    let container = wcontainers_kernel_test::driver::Container {
        files: &files,
        path: &path,
        arguments: &all,
        environment: wcontainers_kernel_test::reference::ENVIRONMENT,
        working_directory: "/",
        backend: None,
        cached: false,
        published: &[],
        address_space: wcontainers_kernel_test::standin::SPACE_32_BIT,
    };
    wcontainers_kernel_test::schedule_seeds()
        .map(|seed| {
            wcontainers_kernel_test::driver::run_outside(
                &container,
                outside.clone(),
                seed,
                Default::default(),
            )
            .expect("an outcome on our kernel")
        })
        .collect()
}

/// The size a terminal starts at when the test gives the client one.
const COLUMNS: u16 = 80;
const ROWS: u16 = 24;

fn terminal(input: bool, script: Vec<Step>) -> Outside {
    Outside {
        input,
        terminal: Some((COLUMNS, ROWS)),
        script,
        ..Default::default()
    }
}

#[test]
fn a_terminal_without_input() {
    // `docker run -t` with no terminal at the client has a terminal of no size.
    let outside = Outside {
        terminal: Some((0, 0)),
        ..Default::default()
    };
    let runs = assert_same_streams(&out(), Some("terminal"), &["info"], &outside);
    only_warnings(&runs, &[reason::PROC_FIELDS_NOT_TRACKED]);
}

#[test]
fn a_terminal_is_resized() {
    let outside = terminal(
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
    assert_same_streams(&out(), Some("terminal"), &["resize", "100", "30"], &outside);
}

#[test]
fn canonical_and_raw_input_with_echo() {
    let outside = terminal(
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
    assert_same_streams(&out(), Some("terminal"), &["lines"], &outside);
}

#[test]
fn interrupt_and_suspend_reach_the_foreground_group() {
    let outside = terminal(
        true,
        vec![
            expect("ready"),
            send("\x03"),
            expect("interrupted"),
            send("\x1a"),
            expect("child"),
        ],
    );
    assert_same_streams(&out(), Some("terminal"), &["signals"], &outside);
}

#[test]
fn a_shell_on_the_terminal() {
    let outside = terminal(
        true,
        vec![
            expect("# "),
            send("sleep 30\r"),
            expect("sleep 30\r\n"),
            send("\x03"),
            expect("^C"),
            expect("# "),
            send("echo $((6*7))\r"),
            expect("42"),
            send("exit\r"),
        ],
    );
    assert_same_streams(&out(), None, &["sh"], &outside);
}

#[test]
fn an_outside_sending_past_its_credit_is_cut_off() {
    let sent = text(7, 1 << 20);
    let outside = Outside {
        input: true,
        script: send_all(sent.clone()),
        excess: 100,
        ..Default::default()
    };
    for run in run_ours("stdin", &["all", "4096", "0"], outside) {
        let past: u64 = run
            .counters
            .iter()
            .filter(|counter| counter.reason == reason::STREAM_PAST_CREDIT)
            .map(|counter| counter.count)
            .sum();
        assert!(past > 0, "the bytes past the credit were counted");
        let text = String::from_utf8_lossy(&run.outcome.stdout);
        let total: usize = text
            .split_whitespace()
            .skip_while(|word| *word != "total")
            .nth(1)
            .and_then(|total| total.parse().ok())
            .expect("a total");
        assert!(total < sent.len(), "only what the credit allowed came in");
        assert!(
            run.warnings
                .iter()
                .any(|warned| warned.reason == reason::STREAM_PAST_CREDIT)
        );
    }
}

#[test]
fn an_outside_that_never_reads_holds_the_writer_to_the_pipe() {
    let outside = Outside {
        output_pace: Some(0),
        ..Default::default()
    };
    for run in run_ours("stdout", &["fill"], outside) {
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
