# Test fixtures

Tiny inputs are checked in. Larger inputs (≥ 64 KiB) are generated
deterministically at test time from xorshift seeds — see
[crates/libsais-golden/tests/corpora.rs](../../crates/libsais-golden/tests/corpora.rs).
