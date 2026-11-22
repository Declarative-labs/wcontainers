use wcontainers_translator::{LookAhead, Optimise, ProgressCount, translate};
use wcontainers_translator_test::guest::x86_64::V3_CPU;
use wcontainers_translator_test::programs::{Library, Position, build_for};
use wcontainers_x86::X86_64;

#[test]
fn scratch_v3() {
    let level = std::env::var("L").unwrap_or("-O2".into());
    let blob = build_for::<X86_64>("bench", &level, Library::None, &[], Position::Fixed, Some(V3_CPU));
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).unwrap();
    let t = translate::<X86_64>(
        &blob.bytes,
        0..blob.code_bytes,
        blob.link_address,
        &[0],
        LookAhead::Full,
        Optimise::Speed,
        ProgressCount::Absent,
        wcontainers_translator_test::runner::probe::<X86_64>(),
        v3,
    )
    .unwrap();
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/scratch-v3.bin"), &blob.bytes).unwrap();
    for (i, m) in t.modules.iter().enumerate() {
        std::fs::write(format!("{}/../../target/scratch-v3-{i}.wasm", env!("CARGO_MANIFEST_DIR")), &m.wasm).unwrap();
        let err = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&m.wasm);
        let Err(e) = err else { continue };
        let at = e.offset();
        eprintln!("module {i}: {e} at {at}");
        let mut function = 0;
        for payload in wasmparser::Parser::new(0).parse_all(&m.wasm) {
            if let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload {
                let range = body.range();
                if range.contains(&at) {
                    eprintln!("function body {function} {range:?}");
                    let mut ops = body.get_operators_reader().unwrap();
                    let mut all = Vec::new();
                    while !ops.eof() {
                        let (op, off) = ops.read_with_offset().unwrap();
                        all.push((off, format!("{op:?}")));
                    }
                    let n = all.iter().position(|(o, _)| *o >= at).unwrap_or(all.len());
                    // Walk back to the opening of the block that ends at `n`, printing its own ops only.
                    let mut depth = 0i32;
                    let mut own = Vec::new();
                    let mut k = n;
                    while k > 0 {
                        k -= 1;
                        let s = &all[k].1;
                        let opens = s.starts_with("Block") || s.starts_with("Loop") || s.starts_with("If");
                        if s == "End" {
                            depth += 1;
                        } else if opens {
                            if depth == 0 {
                                own.push(all[k].clone());
                                break;
                            }
                            depth -= 1;
                        } else if depth == 0 || (depth == 1 && s.starts_with("Else")) {
                            own.push(all[k].clone());
                        }
                        if depth == 1 && s == "End" {
                            own.push(all[k].clone());
                        }
                    }
                    own.reverse();
                    for (o, s) in &own {
                        eprintln!("own {o}: {s}");
                    }
                }
                function += 1;
            }
        }
    }
}
