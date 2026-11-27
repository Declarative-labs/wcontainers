//! The spike's attack on the object table's size, as a test that must pass: more live objects than an old table
//! held, then an object of another kind; and, with the `small-table` feature, more than the table holds at all.
#![forbid(unsafe_code)]

use wcontainers_direct3d_11_direct_harness::Entry;
use wcontainers_direct3d_11_direct_harness::front::Front;

#[cfg(not(feature = "small-table"))]
#[test]
fn the_table_grows_past_the_old_limit() {
    let (full, report) = Entry::served(Front).fill_table(70_000);
    println!("ATTACK table-full: {report}");
    assert_eq!((full.ok_but_null, full.made, full.refused, full.query_made), (0, 70_000, 0, 1));
}

#[cfg(feature = "small-table")]
#[test]
fn a_full_table_reports_a_failure_with_null() {
    let (full, report) = Entry::served(Front).fill_table(20_000);
    println!("ATTACK table-full, small table: {report}");
    assert_eq!(full.ok_but_null, 0, "never S_OK with null, nor a failure without null");
    assert!(full.made > 12_000 && full.refused > 0 && full.made + full.refused == 20_000, "{full:?}");
}
