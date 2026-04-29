# Rust Formula Mapping

This document maps the implemented equations to the main Rust functions in
`crates/swpt_core/src/lib.rs`.

## Execution Flow

```text
calculate
  -> calculate_model
       -> normalize input parameters
       -> estimate_mutual_inductance
       -> compute_eddy_coefficients
            -> field_abs
            -> region_coefficients
       -> find_optimal_angle
            -> stationarity
       -> loss_breakdown
       -> build_samples
```

`calculate` is the WebAssembly entry point. `calculate_model` is the pure Rust
entry point used by tests and by the browser binding.

## Input Parameters

| Formula symbol | Rust field | Notes |
| --- | --- | --- |
| $f$ | `EddyParams::frequency_hz` | The circuit frequency is normalized to the same value. |
| $\omega$ | `2.0 * PI * frequency_hz` | Used in eddy-current and LCC formulas. |
| $\mu_0$ | `MU0` | Vacuum permeability. |
| $\varepsilon_0$ | `EPS0` | Vacuum permittivity. |
| $\varepsilon_r$ | `EddyParams::relative_permittivity` | $\varepsilon=\varepsilon_r\varepsilon_0$. |
| $\sigma$ | `EddyParams::conductivity_s_per_m` | Seawater conductivity. |
| $r_\mathrm{out}$ | `EddyParams::coil_radius_m` | Outer coil radius. |
| $N$ | `EddyParams::turns` | Number of turns. |
| $d$ | `EddyParams::turn_spacing_m` | Radial turn spacing. |
| $h$ | `EddyParams::coil_gap_m` | Coil gap and middle seawater region height. |
| $r_\mathrm{sea}$ | `EddyParams::seawater_radius_m` | Radial integration limit. |
| $h_\mathrm{side}$ | `EddyParams::side_height_m` | One-side external seawater region height. |
| $L_f$ | `CircuitParams::filter_inductance_h` | LCC filter inductance. |
| $M$ | `CircuitParams::mutual_inductance_h` | Overwritten by the estimate when auto-estimation is enabled. |
| $R$ | `CircuitParams::coil_resistance_ohm` | Coil resistance term. |
| $R_f$ | `CircuitParams::filter_resistance_ohm` | Filter inductor ESR. |
| $R_{cf}$ | `CircuitParams::parallel_cap_resistance_ohm` | Parallel compensation capacitor ESR. |
| $R_c$ | `CircuitParams::series_cap_resistance_ohm` | Series compensation capacitor ESR. |
| $R_\mathrm{dson}$ | `CircuitParams::mosfet_rdson_ohm` | MOSFET on-resistance. |

## Eddy-Current Coefficients

Implemented by `compute_eddy_coefficients`, `field_abs`, and
`region_coefficients`.

Complex propagation term:

$$
u(\lambda)=
\sqrt{\lambda^2-\omega\mu_0(\omega\varepsilon-j\sigma)}
=
\sqrt{\lambda^2-\omega^2\mu_0\varepsilon+j\omega\mu_0\sigma}
$$

Rust location:

```text
compute_eddy_coefficients:
  im_part = omega * MU0 * conductivity
  omega2_mu_eps = omega * omega * MU0 * epsilon
  complex_sqrt(lambda^2 - omega2_mu_eps, im_part)
```

Winding source approximation:

$$
r_\mathrm{mean}=r_\mathrm{out}-\frac{d(N-1)}{2}
$$

$$
S_r = Nr_\mathrm{out}-\frac{dN(N-1)}{2}
$$

Rust location:

```text
compute_eddy_coefficients:
  mean_radius
  turn_radius_sum
```

Discretized Hankel integral:

$$
E(\rho,z)\approx
\sum_k w_k J_1(\lambda_k\rho)J_1(\lambda_k r_\mathrm{mean})
\frac{\lambda_k}{u(\lambda_k)}
\exp[-u(\lambda_k)d_z]
$$

Rust location:

```text
compute_eddy_coefficients:
  kernel_re, kernel_im
  jrho = J1(rho_i * lambda_k)

field_abs:
  exp(-u * distance_z)
  source.dot(exp)
  abs = turn_radius_sum * hypot(real, imag)
```

Region definitions:

$$
z_\mathrm{middle}\in[0,h],\qquad
z_\mathrm{side}\in[-h_\mathrm{side},0]
$$

$$
d_{z,p}=|z|,\qquad d_{z,s}=|h-z|
$$

Rust location:

```text
compute_eddy_coefficients:
  middle_z -> region_coefficients -> A, B
  side_z   -> region_coefficients -> C, D
```

For each region:

$$
I_\mathrm{self} =
\int\!\!\int
\left(|E_p|^2+|E_s|^2\right)\rho\,d\rho\,dz
$$

$$
I_\mathrm{cross} =
\int\!\!\int
2|E_p||E_s|\rho\,d\rho\,dz
$$

$$
F = \frac{2\pi\omega^2\mu_0^2\sigma}{4}
$$

Rust location:

```text
region_coefficients:
  self_integral
  cross_integral
  factor
```

Returned coefficients:

$$
A = FI_{\mathrm{self},m},\quad
B = FI_{\mathrm{cross},m},\quad
C = FI_{\mathrm{self},s},\quad
D = FI_{\mathrm{cross},s}
$$

Total eddy-current coefficient:

$$
K_\mathrm{eddy}(\theta)
=A+B\cos\theta+2C+2D\cos\theta
$$

Rust location:

```text
loss_breakdown:
  eddy_index = e.a + e.b * cos_theta + 2.0 * e.c + 2.0 * e.d * cos_theta

stationarity:
  uses the same A/B/C/D combination in the derivative equation
```

## Mutual Inductance Estimate

Implemented by `estimate_mutual_inductance`.

Single-radius Bessel integral:

$$
M(r,a)=\mu_0\pi r a
\int_0^\infty J_1(kr)J_1(ka)e^{-kz}\,dk
$$

Symmetric planar spiral approximation:

$$
r_\mathrm{in}=r_\mathrm{out}-d(N-1)
$$

$$
M \approx
\frac{\mu_0\pi N^2}{(r_\mathrm{out}-r_\mathrm{in})^2}
\int_0^\infty e^{-kz}
\left[
\int_{r_\mathrm{in}}^{r_\mathrm{out}} rJ_1(kr)\,dr
\right]^2 dk
$$

Rust location:

```text
estimate_mutual_inductance:
  inner_radius
  radii, radius_w
  radial = integral r J1(lambda r) dr
  integral += lambda_weight * exp(-lambda * z) * radial^2
```

When `CircuitParams::auto_estimate_mutual_inductance` is true,
`calculate_model` replaces `CircuitParams::mutual_inductance_h` with the
estimated value before solving losses and angle.

## Automatic Lambda Grid

Implemented by `recommend_lambda_grid`.

The automatic recommendation protects the Hankel integral from under-sampling
the oscillatory factor $J_1(\lambda\rho)J_1(\lambda r_\mathrm{mean})$ when
`seawaterRadiusM` becomes large.

The recommended upper limit is:

$$
\lambda_{\max,\mathrm{rec}} =
\min\left(
3000,\,
\max\left[
1000,\,
\frac{10}{d},\,
\frac{8}{\delta}
\right]
\right)
$$

with:

$$
\delta=\sqrt{\frac{2}{\omega\mu_0\sigma}}
$$

The recommended sample count is:

$$
n_{\lambda,\mathrm{rec}} =
\min\left(
6000,\,
\max\left[
640,\,
\left\lceil
\frac{1.1\,\lambda_{\max,\mathrm{rec}}
(r_\mathrm{sea}+r_\mathrm{mean})}{\pi}
\right\rceil+1
\right]
\right)
$$

Rust location:

```text
EddyParams::normalized:
  recommendation = recommend_lambda_grid(&normalized)
  if auto_lambda_grid:
    n_lambda = recommendation.n_lambda
    lambda_max = recommendation.lambda_max

calculate_model:
  numerical_grid reports used and recommended values
```

## Loss Model

Implemented by `loss_breakdown`.

Fixed transferred power relation:

$$
P_\mathrm{trans}=
\frac{MU^2\sin\theta}{\omega L_f^2}
$$

$$
U^2=
\frac{P_\mathrm{trans}\omega L_f^2}{M\sin\theta}
$$

Rust location:

```text
loss_breakdown:
  u2 = p_trans * omega * lf * lf / (m * sin_theta)
  u_rms = sqrt(u2)
```

Loss terms:

$$
P_\mathrm{coil/filter} =
\frac{2U^2(L_f^2R+M^2R_f)}{\omega^2L_f^4}
$$

$$
P_\mathrm{cap} =
\frac{2U^2\left[(M^2+L_f^2+2ML_f\cos\theta)R_{cf}
+R_cL_f^2\right]}{\omega^2L_f^4}
$$

$$
P_\mathrm{eddy} =
\frac{U^2K_\mathrm{eddy}(\theta)}{L_f^2\omega^2}
$$

$$
P_\mathrm{mosfet} =
\frac{4M^2U^2R_\mathrm{dson}}{\omega^2L_f^4}
$$

Rust location:

```text
loss_breakdown:
  coil_filter_loss
  capacitor_loss
  eddy_loss
  mosfet_loss
  total_loss
  efficiency_pct = 100 * P_trans / (P_trans + total_loss)
```

Displayed percentages:

$$
\mathrm{loss\ share} =
\frac{P_i}{P_\mathrm{loss}}
$$

$$
\mathrm{input\ share} =
\frac{P_i}{P_\mathrm{trans}+P_\mathrm{loss}}
$$

## Optimal Angle

Implemented by `stationarity` and `find_optimal_angle`.

The optimum is the root of the stationarity equation for the normalized total
loss:

$$
\frac{d}{d\theta}L(\theta)=0,\qquad
\frac{\pi}{2}<\theta<\pi
$$

The stationarity function contains the derivative contributions from
coil/filter inductor loss, compensation capacitor loss, MOSFET conduction loss,
and eddy-current loss.

`find_optimal_angle` searches with:

```text
1. uniform scan for a sign-changing bracket
2. bisection when a bracket is found
3. golden-section minimum of abs(stationarity) as a fallback
```

## Plot Samples

Implemented by `build_samples`.

This function evaluates `loss_breakdown` and `stationarity` over a uniform
angle grid. The returned data drives the browser charts:

```text
thetaDeg
efficiencyPct
totalLossW
eddyLossPct
residual
```
