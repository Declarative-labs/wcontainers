// Written once for both guests; each guest's module in `output.rs` includes it.

/// The real binary translated for every probe answer, beside the benchmark set.
pub(super) const EVERY_HOST: &str = "musl";
/// The hosts a set is translated for, each with its label.
pub(super) fn hosts(set: &str) -> Vec<(String, <Cpu as wcontainers_guest_cpu::Guest>::Host)> {
    part::PROBE_ANSWERS
        .iter()
        .map(|&answer| (format!("h{answer}"), Cpu::host_from_probe(answer)))
        .filter(|(_, host)| set == "bench" || set == EVERY_HOST || *host == probe())
        .collect()
}
/// Translates the benchmark set in the form `ordering`, with the thread module.
///
/// The whole program (as `measure_benchmarks` sizes it) and each workload alone, from address 0.
pub(super) fn bench(each: Each<'_>, ordering: AccessOrdering) {
    each(
        THREAD,
        &wcontainers_translator::thread_module::<Cpu>(
            wcontainers_translator::ProgressCount::Absent,
        ),
    );
    let mut blobs = vec![build_with(
        "bench",
        "-O2",
        Library::None,
        &["SCALE=1"],
        Position::Fixed,
    )];
    for k in 1..=8 {
        let workload = format!("WORKLOAD={k}");
        blobs.push(build_with(
            "bench",
            "-O2",
            Library::None,
            &["SCALE=1", &workload],
            Position::Fixed,
        ));
    }
    for blob in &blobs {
        for (host_label, host) in hosts("bench") {
            for (label, look_ahead) in look_aheads() {
                for optimise in SETTINGS {
                    let translation = translate::<Cpu>(
                        &blob.bytes,
                        0..blob.code_bytes,
                        blob.link_address,
                        &[0],
                        look_ahead,
                        optimise,
                        wcontainers_translator::ProgressCount::Absent,
                        ordering,
                        host,
                        <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
                    )
                    .unwrap();
                    lines(
                        each,
                        &format!("{}-{host_label}-{label}-{optimise:?}", blob.name),
                        &translation,
                    );
                }
            }
        }
    }
}
/// Translates a real binary from every start its file offers, in the form `ordering`.
pub(super) fn binary(each: Each<'_>, name: &str, ordering: AccessOrdering) {
    let source = sources()
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no set named {name}"));
    let binary = Binary::read(source.name, &std::fs::read(source.file()).unwrap());
    let all = binary.starts.all();
    let mut start_sets = vec![(String::new(), all, false)];
    for (means, starts) in binary.starts.by_means() {
        let means = format!("-{}", means.replace(' ', "_"));
        start_sets.push((means, starts.iter().copied().collect(), true));
    }
    for (host_label, host) in hosts(name) {
        for (label, look_ahead) in look_aheads() {
            for (means, starts, alone) in &start_sets {
                if *alone && !matches!(label, "full" | "bounded") || starts.is_empty() {
                    continue;
                }
                for optimise in SETTINGS {
                    let translation = translate::<Cpu>(
                        &binary.image,
                        binary.code.clone(),
                        binary.link_address,
                        starts,
                        look_ahead,
                        optimise,
                        wcontainers_translator::ProgressCount::Absent,
                        ordering,
                        host,
                        <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
                    )
                    .unwrap();
                    lines(
                        each,
                        &format!("{name}-{host_label}-{label}{means}-{optimise:?}"),
                        &translation,
                    );
                }
            }
        }
    }
}
pub(super) fn write_output() {
    let directory = std::env::var("WC_OUTPUT_DIR").expect("WC_OUTPUT_DIR names where to write");
    let sets = std::env::var("WC_OUTPUT_SETS").expect("WC_OUTPUT_SETS names the sets");
    std::fs::create_dir_all(&directory).unwrap();
    for set in sets.split(',') {
        let mut out = String::new();
        let mut each = |name: &str, wasm: &[u8]| line(&mut out, name, wasm);
        walk(set, &mut each, access_ordering());
        std::fs::write(format!("{directory}/{set}.txt"), out).unwrap();
        eprintln!("wrote {directory}/{set}.txt");
    }
}
/// Translates every module of one set (`bench` or a binary's name) in the form `ordering`.
pub(super) fn walk(set: &str, each: Each<'_>, ordering: AccessOrdering) {
    match set {
        "bench" => bench(each, ordering),
        name => binary(each, name, ordering),
    }
}
