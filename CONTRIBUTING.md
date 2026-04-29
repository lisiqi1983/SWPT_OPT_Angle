# Contributing

Contributions are welcome when they keep the project focused on a reproducible
SWPT optimal operating angle calculator.

## Development Setup

Install Rust and `wasm-pack`, then run:

```bash
cargo test
cargo fmt
./scripts/build_wasm.sh
```

Serve the browser demo locally with:

```bash
./scripts/serve_web.sh 8080
```

## Change Guidelines

- Keep the Rust core deterministic and independent from the browser UI.
- Document any equation change in `docs/algorithm.md`.
- Add or update tests when changing numerical behavior.
- Keep the UI labels and documentation in English.
- Do not commit local benchmark logs or build artifacts from `target/`.

## Numerical Changes

When a change affects the computed angle, efficiency, losses, or ABCD
coefficients, include the input case and the before/after values in the pull
request description. Use `examples/default_case.json` when a compact
reproducible case is sufficient.
