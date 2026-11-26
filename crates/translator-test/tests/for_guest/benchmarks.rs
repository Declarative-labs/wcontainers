// Written once for both guests; each guest's module in `benchmarks.rs` includes it.

pub(super) fn size(blob: &Blob) {
    let code = &blob.bytes[..blob.code_bytes as usize];
    let mut best = (Duration::MAX, Duration::MAX, Vec::new());
    for _ in 0..5 {
        let start = Instant::now();
        let mut translation = translate::<Cpu>(
            &blob.bytes,
            0..blob.code_bytes,
            blob.link_address,
            &[0],
            LookAhead::Full,
            optimise(),
            wcontainers_translator::ProgressCount::Absent,
            harness::runner::access_ordering(),
            harness::runner::probe(),
            <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
        )
        .unwrap();
        let translating = start.elapsed();
        let compiling = compile_time(&translation.modules[0].wasm);
        best = (
            best.0.min(translating),
            best.1.min(compiling),
            translation.modules.remove(0).wasm,
        );
    }
    let wasm = &best.2;
    eprintln!(
        "{}: {} bytes of code ({} gzipped); full look-ahead translated in {:?} ({:.1} MB/s); {} bytes of WebAssembly ({:.1} per byte of code), {} gzipped ({:.1} per gzipped byte); wasmtime compile {:?}",
        blob.name,
        code.len(),
        gzip(code),
        best.0,
        code.len() as f64 / best.0.as_secs_f64() / 1e6,
        wasm.len(),
        wasm.len() as f64 / code.len() as f64,
        gzip(wasm),
        gzip(wasm) as f64 / gzip(code) as f64,
        best.1
    );
}
pub(super) fn measure_benchmarks() {
    let runs: u32 = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS").map_or((1..=8).collect(), |v| {
        v.split(',').map(|n| n.parse().unwrap()).collect()
    });
    let chrome = std::env::var("WC_BENCH_CHROME").map_or(true, |v| v != "0");
    let repeat: u32 = std::env::var("WC_BENCH_REPEAT").map_or(1, |v| v.parse().unwrap());
    let variants: Vec<(&str, Omit)> = match std::env::var("WC_BENCH_VARIANTS") {
        Ok(v) => v
            .split(',')
            .map(|n| variants()[n.parse::<usize>().unwrap() - 1])
            .collect(),
        Err(_) => variants().to_vec(),
    };
    eprintln!("translated for {:?}", optimise());
    size(&build_with(
        "bench",
        "-O2",
        Library::None,
        &["SCALE=1"],
        Position::Fixed,
    ));
    for k in chosen {
        let name = WORKLOADS[k - 1];
        let workload = format!("WORKLOAD={k}");
        let blob = build_with(
            "bench",
            "-O2",
            Library::None,
            &["SCALE=1", &workload],
            Position::Fixed,
        );
        let expected = native(&blob);
        let native_time = native_seconds(&blob, runs);
        eprintln!("{name}: native {native_time:.3} s (load {})", load());
        for &(label, omit) in &variants {
            let mut best = f64::INFINITY;
            let mut counters = Vec::new();
            for _ in 0..runs {
                let (ran, machine, _, time) =
                    translated_omitting(&blob, (LookAhead::Full, optimise()), omit);
                assert_eq!(ran, expected, "{name} with {label}");
                best = best.min(time.as_secs_f64());
                counters = machine.counters();
            }
            eprintln!("{name}: wasmtime, {label}: counters {counters:?}");
            eprintln!(
                "{name}: wasmtime, {label}: {best:.3} s, {:.2} times native (load {})",
                best / native_time,
                load()
            );
        }
        if !chrome {
            continue;
        }
        for &(label, omit) in &variants {
            // The best time of each run in the same instances.
            let mut best = vec![f64::INFINITY; repeat as usize];
            for _ in 0..runs {
                let report = browser::repeated(&blob, (LookAhead::Full, optimise()), omit, repeat);
                assert_eq!(report.ran, expected, "{name} in Chrome with {label}");
                for (best, time) in best.iter_mut().zip(report.runs) {
                    *best = best.min(time);
                }
            }
            let each: Vec<String> = best
                .iter()
                .map(|t| format!("{t:.3} s ({:.2})", t / native_time))
                .collect();
            eprintln!(
                "{name}: Chrome, {label}: {}, times native in brackets (load {})",
                each.join(", then "),
                load()
            );
        }
    }
}
