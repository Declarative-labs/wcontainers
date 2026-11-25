//! Checks the data file against XED: `XED_BUILD=<XED's obj folder> cargo test -p wcontainers-processor-architecture-x86-64 --test forms_source -- --ignored`.

#[path = "../forms/generation/mod.rs"]
#[allow(dead_code, reason = "the generator and each test use different parts")]
mod generation;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::{env, fs};

use generation::form::{LONG_MODE, convert_database, parse_data_file, render_data_file};
use generation::xed::Database;
use generation::{DATA_FILE, DATABASE_FILE, IFORM_TABLE_FILE, locate_in_crate};
use wcontainers_processor_architecture_x86_64::XED_VERSION;

const XED_BUILD_VARIABLE: &str = "XED_BUILD";
const IGNORED: &str = "needs a built XED: set XED_BUILD to its obj folder";
/// The prefixes of the enumerator names in a row of XED's iform table, in the order of the fields they fill.
const TABLE_PREFIXES: [&str; 4] = [
    "XED_ICLASS_",
    "XED_CATEGORY_",
    "XED_EXTENSION_",
    "XED_ISA_SET_",
];
const INVALID_IFORM: &str = "INVALID";

fn locate_xed_build() -> PathBuf {
    env::var_os(XED_BUILD_VARIABLE)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{IGNORED}"))
}

fn read_database() -> Database {
    let text = fs::read_to_string(locate_xed_build().join(DATABASE_FILE))
        .expect("XED's database is readable");
    Database::parse(&text).expect("XED's database parses")
}

#[test]
#[ignore = "needs a built XED: set XED_BUILD to its obj folder"]
fn data_file_is_what_xed_gives() {
    let database = read_database();
    assert_eq!(database.version, XED_VERSION);
    let listing = convert_database(&database).expect("every record converts");
    let recorded =
        fs::read_to_string(locate_in_crate(DATA_FILE)).expect("the data file is readable");
    assert!(
        render_data_file(&listing.forms).expect("the records serialize") == recorded,
        "{DATA_FILE} is not what XED {} gives: run the generator",
        database.version
    );
    println!(
        "{} records; {} forms in the list; {} only in 16-bit or 32-bit mode",
        database.records.len(),
        listing.forms.len(),
        listing.legacy_mode_only.len()
    );
}

/// Returns XED's iform table: each iform with its class, category, extension and ISA set.
fn read_iform_table() -> BTreeMap<String, [String; 4]> {
    let text = fs::read_to_string(locate_xed_build().join(IFORM_TABLE_FILE))
        .expect("XED's iform table is readable");
    text.lines()
        .filter_map(|line| {
            let (iform, row) = line.trim_start().strip_prefix("/*")?.split_once("*/")?;
            let words: Vec<&str> = row
                .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .collect();
            let fields = TABLE_PREFIXES.map(|prefix| {
                words
                    .iter()
                    .find_map(|word| word.strip_prefix(prefix))
                    .unwrap_or_else(|| panic!("{iform} has a {prefix} field"))
                    .to_owned()
            });
            Some((iform.trim().to_owned(), fields))
        })
        .filter(|(iform, _)| iform != INVALID_IFORM)
        .collect()
}

#[test]
#[ignore = "needs a built XED: set XED_BUILD to its obj folder"]
fn forms_are_the_iforms_of_xed_decoder_valid_in_64_bit_mode() {
    let table = read_iform_table();
    let database = read_database();
    let long_mode: BTreeSet<&str> = database
        .records
        .iter()
        .filter(|record| record.mode_restriction.contains(&LONG_MODE))
        .map(|record| record.iform.as_str())
        .collect();
    let text = fs::read_to_string(locate_in_crate(DATA_FILE)).expect("the data file is readable");
    let forms = parse_data_file(&text).expect("every line of the data file is a record");
    let listed: BTreeSet<&str> = forms.iter().map(|form| form.iform.as_str()).collect();
    assert_eq!(
        listed, long_mode,
        "the list holds exactly the iforms with an encoding valid in 64-bit mode"
    );
    let legacy_mode_only: BTreeSet<&str> = table
        .keys()
        .map(String::as_str)
        .filter(|iform| !long_mode.contains(iform))
        .collect();
    assert_eq!(
        listed.len() + legacy_mode_only.len(),
        table.len(),
        "every iform of XED's table is in the list or only in 16-bit or 32-bit mode"
    );
    for form in &forms {
        let fields = table
            .get(&form.iform)
            .unwrap_or_else(|| panic!("{} is in XED's table", form.iform));
        // The C enumerators are the database's names in upper case.
        assert_eq!(
            [&form.iclass, &form.category, &form.extension, &form.isa_set]
                .map(|field| field.to_ascii_uppercase()),
            *fields,
            "{}",
            form.iform
        );
    }
    println!(
        "{} iforms in XED's table: {} in the list, {} only in 16-bit or 32-bit mode",
        table.len(),
        listed.len(),
        legacy_mode_only.len()
    );
}
