wcontainers_capability_table_conformance::tests!((
    wcontainers_capability_table_ordered_map::make_shard_table as fn() -> _,
    u64::from as fn(u8) -> u64,
    (|number: u8| wcontainers_memory_shard::MemoryShardConfig {
        length: u64::from(number),
        permissions: wcontainers_memory_shard::MemoryPermissions {
            read: true,
            write: true,
            execute: false
        },
    }) as fn(u8) -> wcontainers_memory_shard::MemoryShardConfig,
    (|| wcontainers_memory_shard::MemoryShardConfig {
        length: u64::MAX,
        permissions: wcontainers_memory_shard::MemoryPermissions {
            read: true,
            write: true,
            execute: false
        },
    }) as fn() -> wcontainers_memory_shard::MemoryShardConfig,
    (|shard: &wcontainers_capability_table_ordered_map::Shard| {
        u8::try_from(wcontainers_memory_shard::MemoryShard::len(shard))
            .expect("every shard is as long as its number")
    }) as fn(&wcontainers_capability_table_ordered_map::Shard) -> u8,
    (|shard: &wcontainers_capability_table_ordered_map::Shard, mark: u8| {
        let length = wcontainers_memory_shard::MemoryShard::len(shard);
        let bytes = wcontainers_memory_shard::MemoryShard::borrow(shard, u64::MIN, length)
            .expect("a shard lends itself");
        assert_eq!(
            wcontainers_volatile_bytes::VolatileBytes::write_at(&bytes, u64::MIN, &[mark]),
            [mark].len()
        );
    }) as fn(&wcontainers_capability_table_ordered_map::Shard, u8),
    (|shard: &wcontainers_capability_table_ordered_map::Shard| {
        let length = wcontainers_memory_shard::MemoryShard::len(shard);
        let bytes = wcontainers_memory_shard::MemoryShard::borrow(shard, u64::MIN, length)
            .expect("a shard lends itself");
        let mut first = [u8::MIN];
        assert_eq!(
            wcontainers_volatile_bytes::VolatileBytes::read_at(&bytes, u64::MIN, &mut first),
            first.len()
        );
        let [mark] = first;
        mark
    }) as fn(&wcontainers_capability_table_ordered_map::Shard) -> u8,
));
