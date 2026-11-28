wcontainers_capability_table_conformance::tests!((
    wcontainers_capability_table_ordered_map::make_shard_table as fn() -> _,
    u64::from as fn(u8) -> u64,
    (|number: u8| wcontainers_memory_shard::MemoryShardConfig {
        length: u64::from(number),
        permissions: wcontainers_memory_shard::MemoryPermissions {
            read: number.is_power_of_two(),
            write: !number.is_power_of_two(),
            execute: number.is_power_of_two(),
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
    (|config: &wcontainers_memory_shard::MemoryShardConfig| {
        let permissions = config.permissions;
        (
            config.length,
            permissions.read,
            permissions.write,
            permissions.execute,
        )
    }) as fn(&wcontainers_memory_shard::MemoryShardConfig) -> (u64, bool, bool, bool),
    (|shard: &wcontainers_capability_table_ordered_map::Shard| {
        let permissions = wcontainers_memory_shard::MemoryShard::permissions(shard);
        (
            wcontainers_memory_shard::MemoryShard::len(shard),
            permissions.read,
            permissions.write,
            permissions.execute,
        )
    }) as fn(&wcontainers_capability_table_ordered_map::Shard) -> (u64, bool, bool, bool),
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
