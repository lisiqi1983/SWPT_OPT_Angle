# SWPT Optimal Operating Angle Calculator

This is a Rust + WebAssembly open-source implementation of the seawater wireless
power transfer optimal operating angle algorithm.

The browser runs the calculation from the eddy-current integral to the final
loss breakdown. The computation is not based on precomputed `A/B/C/D` tables.

## What It Computes

- Seawater eddy-current coefficients `A/B/C/D`
- Optimal primary/secondary current phase angle in
  $\frac{\pi}{2}<\theta<\pi$
- Coil/filter inductor loss
- Compensation capacitor loss
- Seawater eddy-current loss
- MOSFET conduction loss
- Efficiency at $\theta=\frac{\pi}{2}$ and at the optimized angle
- Loss percentages relative to total loss and input power
- Optional automatic mutual-inductance estimate from coil radius, turn count,
  turn spacing, and coil gap
- Automatic `lambda` grid recommendation for large integration domains

Absolute efficiency depends strongly on the circuit parameters. The web UI
therefore estimates `M` by default from coil geometry and still allows manual
override when measured or FEM-extracted values are available.

## Repository Layout

```text
swpt_opt_angle_rust/
  crates/swpt_core/     Rust calculation core and WASM bindings
  docs/                 Algorithm and parameter notes
  examples/             Example input payloads
  scripts/              Build and local serving helpers
  web/                  Browser UI and generated WASM package
```

## Implementation

The Rust core uses:

- `ndarray` for matrix products in the discretized Hankel integral
- `libm` for the first-order Bessel function `J1`
- `wasm-bindgen` for browser calls

The fast path replaces per-point adaptive integration with:

$$
E(\rho,z)\approx
\sum_k w_k J_1(\lambda_k\rho)J_1(\lambda_k r_\mathrm{mean})
\frac{\lambda_k}{u(\lambda_k)}
\exp[-u(\lambda_k)d_z]
$$

See `docs/algorithm.md` for the implemented equations and
`docs/rust_formula_mapping.md` for the formula-to-function mapping.

## Run The Prebuilt Demo

The generated `web/pkg` directory is included, so the browser demo can run
without installing Rust, `wasm-pack`, Node.js, or npm.

Serve the web page with any static HTTP server. The included helper script
requires Python 3 because it calls `python3 -m http.server`:

```bash
./scripts/serve_web.sh 8080
```

Open:

```text
http://localhost:8080
```

Opening `web/index.html` directly from the file system is not recommended
because browsers commonly block WebAssembly module loading from `file://` URLs.

If Python 3 is not available, use another static server such as nginx, Caddy,
VS Code Live Server, or any equivalent tool that can serve the `web/`
directory over HTTP.

## GitHub Pages

The repository includes `.github/workflows/pages.yml`, which publishes the
prebuilt static browser demo from `web/`.

To enable the hosted demo on GitHub:

1. Open the repository `Settings`.
2. Open `Pages`.
3. Set `Source` to `GitHub Actions`.
4. Push to `main` or run the `Deploy Pages` workflow manually.

The deployed URL will be available from the workflow summary and normally has
this form:

```text
https://<github-user>.github.io/<repository-name>/
```

## Rebuild From Source

Install Rust only when changing the Rust source code or regenerating `web/pkg`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Build WebAssembly:

```bash
cd swpt_opt_angle_rust
./scripts/build_wasm.sh
```

## Development

After Rust is installed:

```bash
cargo test
cargo fmt
./scripts/build_wasm.sh
```

The browser UI uses plain HTML, CSS, and JavaScript. No frontend package manager
is required for the current demo.

## Current Scope

The implemented model is the coaxial, symmetric-coil case. Misalignment and
offset field mapping should be implemented as a separate model mode in a later
step.

## License

Licensed under the Apache License, Version 2.0. See `LICENSE` and `NOTICE`.
