//! Kani proofs that every call gives the kernel only a region of scratch of the value's own length, and that the
//! requests the primitive builds name only what they should.

use super::*;
use wcontainers_socket_options::StorageBytes;

/// Longer than every option's exact length, so that a value one byte too long is tried for each.
const LONGEST: usize = size_of::<linux_raw_sys::net::tcp_ao_getsockopt>() + 1;

fn any_option() -> PlainOption {
    let index: usize = kani::any();
    kani::assume(index < PlainOption::ALL.len());
    PlainOption::ALL[index]
}

fn any_length() -> usize {
    let length: usize = kani::any();
    kani::assume(length <= LONGEST);
    length
}

fn taken(option: PlainOption, length: usize) -> bool {
    option.value().exact_length().is_none_or(|exact| exact == length)
}

#[kani::proof]
fn longest_is_longer_than_every_exact_length() {
    let option = any_option();
    if let Some(exact) = option.value().exact_length() {
        assert!(exact < LONGEST);
    }
}

#[kani::proof]
fn a_set_reads_only_the_value() {
    let option = any_option();
    let length = any_length();
    let value = vec![u8::MAX; length];
    let result = set(0, option, &value);
    assert_eq!(result.is_ok(), taken(option, length) && option.value() != Value::Filled);
}

#[kani::proof]
fn a_get_writes_only_the_buffer() {
    let option = any_option();
    let length = any_length();
    let mut into = vec![u8::MIN; length];
    match get(0, option, &mut into) {
        Ok(written) => assert!(taken(option, length) && written <= length),
        Err(error) => assert!(!taken(option, length) && error == Errno::INVAL),
    }
}

#[kani::proof]
fn a_count_writes_one_int() {
    let request: u32 = kani::any();
    assert!(read_count(0, request).is_ok());
}

#[kani::proof]
fn a_full_information_layout_keeps_the_arrays_before_the_request() {
    let subflows: usize = kani::any();
    let information: usize = kani::any();
    kani::assume(subflows <= 4 && information <= 4);
    let layout = FullLayout::new::<8, [u8; 3], u64>(subflows, information).unwrap();
    assert!(layout.count <= subflows && layout.count <= information);
    assert_eq!(layout.subflow_size, size_of::<[u8; 3]>());
    assert_eq!(layout.information_size, size_of::<u64>());
    assert!(layout.subflows + layout.count * layout.subflow_size <= layout.information);
    assert!(layout.information + layout.count * layout.information_size <= layout.header);
    assert_eq!(layout.header + size_of::<FullInformation<8>>(), layout.length);
}

#[kani::proof]
fn a_key_query_asks_for_one_key_whatever_it_held() {
    let mut query: [u8; AUTHENTICATION_KEY_QUERY] = kani::any();
    ask_for_one_key(&mut query);
    let at = offset_of!(uapi::tcp_ao_getsockopt, nkeys);
    let asked = u32::from_ne_bytes(query[at..at + size_of::<u32>()].try_into().unwrap());
    assert_eq!(asked, KEYS_ASKED);
}

#[kani::proof]
fn a_source_filter_counts_the_sources_it_carries() {
    let sources = [[u8::MAX; size_of::<StorageBytes>()]; 2];
    let count: usize = kani::any();
    kani::assume(count <= sources.len());
    let filter = SourceFilterOptions {
        level: wcontainers_socket_options::MulticastLevel::Ipv4,
        interface: kani::any(),
        group: [u8::MAX; size_of::<StorageBytes>()],
        mode: kani::any(),
        sources: &sources[..count],
    };
    let request = source_filter_request(&filter);
    type Header = uapi::group_filter__bindgen_ty_1__bindgen_ty_2;
    let start = offset_of!(Header, gf_slist_flex);
    let at = offset_of!(Header, gf_numsrc);
    let carried = u32::from_ne_bytes(request[at..at + size_of::<u32>()].try_into().unwrap());
    assert_eq!(carried as usize, count);
    assert_eq!(request.len(), start + count * size_of::<StorageBytes>());
}
