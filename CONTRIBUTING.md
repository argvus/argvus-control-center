# Contributing to argvus-control-center

Thanks for helping build ARGVUS!

## Ground rules

1. **Core-first**: business logic lives in `argvus-control-center-core` or the
   relevant domain crate (`-settings`, `-hardware`, etc.). The binary is a
   thin dispatch and event loop layer.
2. **No shell execution**: all system interaction uses `Command::new()` with
   explicit argument vectors. Never pipe through `sh -c`.
3. **No new conventions without discussion**: prefer freedesktop/Linux
   standards and existing ARGVUS backends over project-specific mechanisms.
4. **Icons are optional**: every UI label must be readable with icons disabled.
   Use `AppConfig::icon()` and the `non_empty()` fallback pattern.

## Workflow

```sh
fork & clone
cargo fmt
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Commit style (Conventional Commits):

```
feat(settings): short imperative summary
fix(boot): ...
docs(readme): ...
test(core): ...
build(arch): ...
chore: ...
```

Keep each commit focused on one responsibility. Pull requests must pass CI
(formatting, build, tests, clippy) and include tests for any behavior change.

## Reporting issues

Include distribution/version, how you ran the command, the full terminal
output, and what you expected to happen. Never paste private data.
