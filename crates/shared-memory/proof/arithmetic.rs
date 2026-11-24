//! A COPY of ../arithmetic.rs with Verus's clauses added: Verus proves this copy, not the crate's file, and
//! agreement.rs tests that the bodies of the two give the same answers. Run: verus --crate-type=lib arithmetic.rs
#![allow(unused_imports)]
use vstd::prelude::*;

verus! {

pub const WORD: usize = 8;

pub struct Plan {
    pub low: usize,
    pub index: usize,
    pub head: usize,
}

/// What `read` and `write` rely on: the first word is the one `at` lies in, a head that does not fill its word is all
/// the bytes, and the head's word, the whole words after it and the tail's word all lie below `count`.
pub open spec fn fits(count: usize, at: u64, length: usize, p: Plan) -> bool {
    let body = length - p.head;
    &&& p.low == at % 8 && p.index == at / 8
    &&& p.low + p.head <= 8 && p.head <= length
    &&& (p.head < length ==> p.low + p.head == 8 || p.low == 0 && p.head == 0)
    &&& (p.head > 0 ==> p.index < count)
    &&& p.index + (if p.head > 0 { 1int } else { 0 }) + body / 8 + (if body % 8 > 0 { 1int } else { 0 }) <= count
}

pub fn plan(count: usize, at: u64, length: usize) -> (r: Option<Plan>)
    requires
        count * 8 <= u64::MAX,
    ensures
        r is Some <==> at + length <= count * 8,
        r matches Some(p) ==> fits(count, at, length, p),
{
    if at.checked_add(length as u64)? > count as u64 * WORD as u64 {
        return None;
    }
    let low = (at % WORD as u64) as usize;
    let room = (WORD - low) % WORD;
    Some(Plan {
        low,
        index: (at / WORD as u64) as usize,
        head: if room < length { room } else { length },
    })
}

pub fn part(count: usize, start: u64, end: u64) -> (r: Option<(u64, u64)>)
    requires
        count * 8 <= u64::MAX,
    ensures
        r is Some <==> start <= end <= count * 8,
        r matches Some((s, l)) ==> s == start && s + l == end,
{
    if start > end || end > count as u64 * WORD as u64 {
        return None;
    }
    Some((start, end - start))
}

} // verus!
