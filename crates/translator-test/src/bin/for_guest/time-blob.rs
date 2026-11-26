// Written once for both guests; each guest's module in `time-blob.rs` includes it.

pub(super) fn dump(program: &str, optimisation: &str, out: &str, defines: &[&str]) {
    let blob = build_with(
        program,
        optimisation,
        Library::None,
        defines,
        Position::Fixed,
    );
    let mut bytes = blob.code_bytes.to_le_bytes().to_vec();
    bytes.extend(blob.link_address.expect("a fixed blob").to_le_bytes());
    bytes.extend(&blob.bytes);
    std::fs::write(out, bytes).unwrap();
}
pub(super) fn read(file: &str) -> Blob {
    let bytes = std::fs::read(file).unwrap();
    let (size, rest) = bytes.split_at(HEADER_WORD);
    let (link, rest) = rest.split_at(HEADER_WORD);
    Blob {
        name: std::path::Path::new(file)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        bytes: rest.to_vec(),
        code_bytes: u64::from_le_bytes(size.try_into().unwrap()),
        load: LOAD,
        link_address: Some(u64::from_le_bytes(link.try_into().unwrap())),
        main: None,
        program_headers: None,
        stack_bytes: STACK_BYTES,
        offer: None,
    }
}
/// One timed run: the output, the status and the seconds the guest ran.
pub(super) fn once(how: &str, blob: &Blob) -> (Vec<u8>, i32, f64) {
    match how {
        NATIVE => {
            let ran = native(blob);
            (ran.stdout, ran.status, native_seconds(blob, 1))
        }
        WASMTIME => {
            let (ran, _, _, time) = translated_omitting(blob, HOW, Omit::default());
            (ran.stdout, ran.status, time.as_secs_f64())
        }
        CHROME => {
            let report = browser::translated_with(blob, HOW, Omit::default());
            let seconds = report.runs.iter().copied().fold(f64::INFINITY, f64::min);
            (report.ran.stdout, report.ran.status, seconds)
        }
        _ => panic!("time natively, in wasmtime or in Chrome, not {how}"),
    }
}
pub(super) fn time(how: &str, file: &str, runs: u32) {
    let blob = read(file);
    let mut best = f64::INFINITY;
    let mut outcome = None;
    for _ in 0..runs {
        let (out, status, seconds) = once(how, &blob);
        best = best.min(seconds);
        outcome = Some((out, status));
    }
    let (out, status) = outcome.expect("one run at least");
    let out = String::from_utf8_lossy(&out);
    println!("{file} {how}: status {status}, best of {runs} {best:.3} s, output {out:?}");
}
pub(super) fn rounds(rounds: u32, files: &[&str]) {
    for file in files {
        let blob = read(file);
        let mut best = [f64::INFINITY; EACH_ROUND.len()];
        let mut outputs = Vec::new();
        for round in 0..rounds {
            for (how, best) in EACH_ROUND.iter().zip(&mut best) {
                let load = load();
                let (out, status, seconds) = once(how, &blob);
                println!(
                    "{file} round {round} {how}: {seconds:.3} s, status {status} (load {load})"
                );
                *best = best.min(seconds);
                outputs.push((out, status));
            }
        }
        assert!(
            outputs.windows(2).all(|pair| pair[0] == pair[1]),
            "{file}: every run gives the native output"
        );
        let [native, ..] = best;
        let each: String = EACH_ROUND
            .iter()
            .zip(best)
            .map(|(how, best)| format!(" {how} {best:.3} s ({:.2});", best / native))
            .collect();
        println!("{file}: best of {rounds}:{each} times native in brackets");
    }
}
