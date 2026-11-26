//! The recording back end's own tests: the interface's rules it enforces, the fast-path defaults and the
//! shared helpers it runs, and random call sequences it records and replays.

mod compute;
mod fast_paths;
mod helpers;
mod recorder;
mod rules;
mod sequences;
mod standalone;
