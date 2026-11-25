# Conventions

Set by the owner on 2026-11-18. They apply to every crate, test and document in this repository.

- Names say what the thing is in the precise technical term, with as many words as it takes to be
  unambiguous: `Serial`, `ParallelOptimistic`, `ThreadOrdering`, `ReplayDeterminism`, `GuestMachine`,
  `TranslatorConfig`. A getter is a noun phrase and never reads as a verb (`replay_determinism()`).
  Vague words are rejected: outside, back end, engine, tuning, room, grant, protection, view.
- A long-lived configuration type ends in `Config` (`RuntimeConfig`, `TranslatorConfig`); a type holding
  the choices for one call ends in `Options`.
- A hook is something the kernel announces: on the `System` trait, synchronous, returning nothing, named
  `on_` plus the event, in the -ing form when it has not happened yet (`on_process_starting`) and the past
  form when it has (`on_inspection_ended`). A service returns a value and is a plain verb.
- Comments are few: types and names carry the meaning. A doc comment is one sentence, and only where the
  name cannot say it.
- No magic numbers or literals: every number, and every string a program interprets, is derived from what
  it describes, imported from the crate that defines it, or one named constant defined once.
- Standard-library and established-crate types come before our own (`std::ops::Range<u64>`, not a range
  struct).
- Code made longer or more indirect only because Verus cannot read the plain Rust form carries
  `// VERUS: <what Verus cannot read>, so <what is written instead>.` on the line above, so it can be
  undone when Verus catches up.
