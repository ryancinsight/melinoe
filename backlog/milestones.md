# Milestone ladder ([ADR 0002](../docs/adr/0002-release-scope-1-0.md))
<a id="M-branded-collections"></a> **M-branded-collections** (0.11.0) needs: (none) — branded collection access and partitioning.
- exit: borrow and mutate branded vectors through permits (`cargo test --locked --all-features --test branded_vec` passes).
- exit: operate branded deques with value-semantic correctness (`cargo test --locked --all-features --test branded_deque` passes).
- exit: partition branded data for concurrent reads and writes (`cargo test --locked --all-features --test partition` and `cargo test --locked --all-features --test par_chunks` pass).
- exit: demonstrate branded-cell usage (`cargo run --locked --example book_branded_cell --all-features` exits successfully).
- standing: no open correctness item.
<a id="M-1.0"></a> **M-1.0** (1.0.0) needs: M-branded-collections — stable capability release (authority-gated).
- exit: document and exercise conditional atomic and collection contracts (`cargo test --locked --all-features --test conditional_atomics` and `cargo run --locked --example book_conditional_atomics --all-features` pass).
- standing: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, `cargo test --locked --all-features`, `cargo test --locked --doc --all-features`, and `cargo doc --locked --no-deps --all-features` pass; SemVer and publish dry-run gates required for the declared release pass; README and rustdoc are current; no open correctness item.
