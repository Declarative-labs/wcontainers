use wcontainers_random_source::StandaloneRandomSource;
use wcontainers_random_source_seeded::{SeededRandomSource, SeededRandomSourceConfig};

wcontainers_random_source_conformance::tests!(SeededRandomSource::new(SeededRandomSourceConfig {
    seed: u64::MIN
}));
