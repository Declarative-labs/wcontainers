//! Kani proofs that every listed option, with a value of any length, passes the kernel only bytes of its own buffer.

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
    match option.value_length() {
        ValueLength::Exactly(exact) => exact == length,
        ValueLength::Any => true,
    }
}

#[kani::proof]
fn longest_is_longer_than_every_exact_length() {
    let option = any_option();
    if let ValueLength::Exactly(exact) = option.value_length() {
        assert!(exact < LONGEST);
    }
}

#[kani::proof]
fn a_set_reads_only_the_value() {
    let option = any_option();
    let length = any_length();
    let value = vec![u8::MAX; length];
    let result = set(0, option, &value);
    assert_eq!(result.is_ok(), taken(option, length));
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
fn a_full_information_request_names_only_the_arrays() {
    let subflows: usize = kani::any();
    let information: usize = kani::any();
    kani::assume(subflows <= 4 && information <= 4);
    let mut subflow_array = vec![[u8::MIN; 3]; subflows];
    let mut information_array = vec![u64::MIN; information];
    let header =
        FullInformation::<8>::new(&mut subflow_array[..], &mut information_array[..]).unwrap();
    let sizes = header.sizes;
    assert!(sizes[ARRAYS_USER] as usize <= subflows && sizes[ARRAYS_USER] as usize <= information);
    assert_eq!(sizes[SUBFLOW_INFO_USER] as usize, size_of::<[u8; 3]>());
    assert_eq!(sizes[TCP_INFO_USER] as usize, size_of::<u64>());
    let (subflow_info, tcp_info) = (header.subflow_info, header.tcp_info);
    assert_eq!(subflow_info, subflow_array.as_ptr().addr() as u64);
    assert_eq!(tcp_info, information_array.as_ptr().addr() as u64);
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
