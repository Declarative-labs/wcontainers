//! GPU virtual addresses and descriptor handles: numbers the program computes with, which the front
//! turns back into its own objects.
//!
//! Buffers get address ranges of their own (`GetGPUVirtualAddress`); vertex and index buffer views,
//! root descriptors and indirect arguments name an address inside one, and [`AddressMap::find`]
//! gives the buffer and the offset. Descriptor heaps get handle ranges the same way: a handle is the
//! heap's start plus the index times [`DESCRIPTOR_SIZE`].

use std::collections::BTreeMap;

/// The step between descriptors of every heap type (`GetDescriptorHandleIncrementSize`).
pub const DESCRIPTOR_SIZE: u64 = 64;
/// Alignment of each range handed out, as resources are placed at 64 KiB
/// (`D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT`). Zero is never handed out.
const ALIGN: u64 = 1 << 16;

/// Ranges of numbers, each owned by one object `T`.
#[derive(Debug)]
pub struct AddressMap<T> {
    next: u64,
    ranges: BTreeMap<u64, (u64, T)>,
}

impl<T: Copy> AddressMap<T> {
    /// A map handing out numbers from `base` upwards; separate maps use distant bases so a number of
    /// one kind is never taken for another.
    pub fn new(base: u64) -> Self {
        AddressMap {
            next: base.max(ALIGN),
            ranges: BTreeMap::new(),
        }
    }

    /// Hands out a range of `size` bytes owned by `owner`, and returns its start.
    pub fn insert(&mut self, size: u64, owner: T) -> u64 {
        let start = self.next;
        self.next = (start + size.max(1)).next_multiple_of(ALIGN);
        self.ranges.insert(start, (start + size.max(1), owner));
        start
    }

    /// The owner of the range holding `address`, and the offset into it.
    pub fn find(&self, address: u64) -> Option<(T, u64)> {
        let (start, (end, owner)) = self.ranges.range(..=address).next_back()?;
        (address < *end).then(|| (*owner, address - start))
    }

    /// Forgets the range starting at `start`. Its numbers are not handed out again.
    pub fn remove(&mut self, start: u64) {
        self.ranges.remove(&start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_find_their_owner_and_offset() {
        let mut map = AddressMap::new(1 << 40);
        let a = map.insert(100, 'a');
        let b = map.insert(ALIGN * 2 + 1, 'b');
        assert_eq!(a % ALIGN, 0);
        assert!(b >= a + 100 && b % ALIGN == 0);
        assert_eq!(map.find(a), Some(('a', 0)));
        assert_eq!(map.find(a + 99), Some(('a', 99)));
        assert_eq!(map.find(a + 100), None);
        assert_eq!(map.find(b + ALIGN * 2), Some(('b', ALIGN * 2)));
        assert_eq!(map.find(0), None);
        map.remove(a);
        assert_eq!(map.find(a), None);
        assert!(map.insert(1, 'c') > b);
    }

    #[test]
    fn descriptor_handles_step_by_the_increment() {
        let mut heaps = AddressMap::new(1 << 20);
        let start = heaps.insert(8 * DESCRIPTOR_SIZE, 7u32);
        assert_eq!(
            heaps.find(start + 5 * DESCRIPTOR_SIZE),
            Some((7, 5 * DESCRIPTOR_SIZE))
        );
    }
}
