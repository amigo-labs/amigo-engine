## Acceptance Criteria for: <Feature/Change Name>

### API Completeness
<!-- One criterion per public type, method, or function from the spec/ADR -->
- [ ] ...

### Behavior
<!-- One criterion per described behavior, edge case, and error condition -->
- [ ] ...

### Quality Gates
<!-- These are always required -->
- [ ] `cargo check --workspace` compiles without errors
- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] `cargo fmt --all --check` — correctly formatted
- [ ] New public API has at least one test per method
- [ ] No `unwrap()` in library code
- [ ] No `todo!()` or `unimplemented!()` in committed code

### Convention Compliance
<!-- Add criteria based on the project's conventions -->
- [ ] ...
