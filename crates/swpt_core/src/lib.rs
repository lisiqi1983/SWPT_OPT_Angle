use ndarray::{Array1, Array2, Axis, Zip};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const PI: f64 = std::f64::consts::PI;
const MU0: f64 = 4.0 * PI * 1.0e-7;
const EPS0: f64 = 8.85e-12;
const DEFAULT_LAMBDA_MAX: f64 = 1000.0;
const MIN_RECOMMENDED_N_LAMBDA: usize = 640;
const MAX_RECOMMENDED_N_LAMBDA: usize = 6000;
const AUTO_LAMBDA_MAX_LIMIT: f64 = 3000.0;
const LAMBDA_GRID_SAFETY: f64 = 1.1;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn calculate(input: JsValue) -> Result<JsValue, JsValue> {
    let input: ModelInput =
        serde_wasm_bindgen::from_value(input).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let result = calculate_model(input).map_err(|err| JsValue::from_str(&err))?;
    serde_wasm_bindgen::to_value(&result).map_err(|err| JsValue::from_str(&err.to_string()))
}

pub fn calculate_model(input: ModelInput) -> Result<CalculationResult, String> {
    let eddy_params = input.eddy.normalized();
    let mut circuit = input
        .circuit
        .normalized_with_frequency(eddy_params.frequency_hz);
    let options = input.options.normalized();
    let estimated_mutual_inductance_h = estimate_mutual_inductance(&eddy_params)?;
    if circuit.auto_estimate_mutual_inductance {
        circuit.mutual_inductance_h = estimated_mutual_inductance_h;
    }

    let t0 = js_now();
    let coeffs = compute_eddy_coefficients(&eddy_params)?;
    let t1 = js_now();
    let optimum = find_optimal_angle(&circuit, &coeffs);
    let t2 = js_now();
    let optimum_loss = loss_breakdown(optimum.theta_rad, &circuit, &coeffs);
    let at_90 = loss_breakdown(PI / 2.0, &circuit, &coeffs);
    let samples = build_samples(&circuit, &coeffs, options.sample_count);

    Ok(CalculationResult {
        coefficients: coeffs,
        optimum,
        optimum_loss,
        at_90,
        samples,
        numerical_grid: NumericalGrid {
            n_rho: eddy_params.n_rho,
            n_z: eddy_params.n_z,
            n_lambda: eddy_params.n_lambda,
            lambda_max: eddy_params.lambda_max,
            lambda_step: lambda_step(eddy_params.lambda_max, eddy_params.n_lambda),
            auto_lambda_grid: eddy_params.auto_lambda_grid,
            recommended_n_lambda: eddy_params.recommended_n_lambda,
            recommended_lambda_max: eddy_params.recommended_lambda_max,
        },
        timings_ms: Timings {
            eddy: t1 - t0,
            angle: t2 - t1,
            total: js_now() - t0,
        },
        estimated_mutual_inductance_h,
        used_mutual_inductance_h: circuit.mutual_inductance_h,
        notes: vec![
            "Coaxial symmetric-coil model.".to_string(),
            "Loss values assume constant target transferred power.".to_string(),
            "Matrix acceleration uses ndarray for Hankel-kernel products.".to_string(),
        ],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelInput {
    pub eddy: EddyParams,
    pub circuit: CircuitParams,
    pub options: SolverOptions,
}

impl Default for ModelInput {
    fn default() -> Self {
        Self {
            eddy: EddyParams::default(),
            circuit: CircuitParams::default(),
            options: SolverOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EddyParams {
    pub frequency_hz: f64,
    pub conductivity_s_per_m: f64,
    pub relative_permittivity: f64,
    pub coil_radius_m: f64,
    pub turns: usize,
    pub turn_spacing_m: f64,
    pub coil_gap_m: f64,
    pub seawater_radius_m: f64,
    pub side_height_m: f64,
    pub n_rho: usize,
    pub n_z: usize,
    pub auto_lambda_grid: bool,
    pub n_lambda: usize,
    pub lambda_max: f64,
    #[serde(skip)]
    pub recommended_n_lambda: usize,
    #[serde(skip)]
    pub recommended_lambda_max: f64,
}

impl Default for EddyParams {
    fn default() -> Self {
        Self {
            frequency_hz: 200.0e3,
            conductivity_s_per_m: 4.0,
            relative_permittivity: 81.0,
            coil_radius_m: 0.2,
            turns: 14,
            turn_spacing_m: 0.011,
            coil_gap_m: 0.12,
            seawater_radius_m: 0.35,
            side_height_m: 0.48,
            n_rho: 64,
            n_z: 64,
            auto_lambda_grid: true,
            n_lambda: 640,
            lambda_max: DEFAULT_LAMBDA_MAX,
            recommended_n_lambda: 640,
            recommended_lambda_max: DEFAULT_LAMBDA_MAX,
        }
    }
}

impl EddyParams {
    fn normalized(&self) -> Self {
        let defaults = Self::default();
        let mut normalized = Self {
            frequency_hz: positive(self.frequency_hz, defaults.frequency_hz),
            conductivity_s_per_m: positive(
                self.conductivity_s_per_m,
                defaults.conductivity_s_per_m,
            ),
            relative_permittivity: positive(
                self.relative_permittivity,
                defaults.relative_permittivity,
            ),
            coil_radius_m: positive(self.coil_radius_m, defaults.coil_radius_m),
            turns: self.turns.clamp(1, 200),
            turn_spacing_m: positive(self.turn_spacing_m, defaults.turn_spacing_m),
            coil_gap_m: positive(self.coil_gap_m, defaults.coil_gap_m),
            seawater_radius_m: positive(self.seawater_radius_m, defaults.seawater_radius_m),
            side_height_m: positive(self.side_height_m, defaults.side_height_m),
            n_rho: self.n_rho.clamp(8, 300),
            n_z: self.n_z.clamp(8, 300),
            auto_lambda_grid: self.auto_lambda_grid,
            n_lambda: self.n_lambda.clamp(40, MAX_RECOMMENDED_N_LAMBDA),
            lambda_max: positive(self.lambda_max, defaults.lambda_max),
            recommended_n_lambda: defaults.recommended_n_lambda,
            recommended_lambda_max: defaults.recommended_lambda_max,
        };
        let recommendation = recommend_lambda_grid(&normalized);
        normalized.recommended_n_lambda = recommendation.n_lambda;
        normalized.recommended_lambda_max = recommendation.lambda_max;
        if normalized.auto_lambda_grid {
            normalized.n_lambda = recommendation.n_lambda;
            normalized.lambda_max = recommendation.lambda_max;
        }
        normalized
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CircuitParams {
    pub frequency_hz: f64,
    pub auto_estimate_mutual_inductance: bool,
    pub transferred_power_w: f64,
    pub filter_inductance_h: f64,
    pub mutual_inductance_h: f64,
    pub coil_resistance_ohm: f64,
    pub filter_resistance_ohm: f64,
    pub parallel_cap_resistance_ohm: f64,
    pub series_cap_resistance_ohm: f64,
    pub mosfet_rdson_ohm: f64,
}

impl Default for CircuitParams {
    fn default() -> Self {
        Self {
            frequency_hz: 200.0e3,
            auto_estimate_mutual_inductance: true,
            transferred_power_w: 1000.0,
            filter_inductance_h: 15.0e-6,
            mutual_inductance_h: 2.0e-6,
            coil_resistance_ohm: 0.3,
            filter_resistance_ohm: 0.07,
            parallel_cap_resistance_ohm: 0.05,
            series_cap_resistance_ohm: 0.05,
            mosfet_rdson_ohm: 0.045,
        }
    }
}

impl CircuitParams {
    fn normalized_with_frequency(&self, frequency_hz: f64) -> Self {
        let defaults = Self::default();
        Self {
            frequency_hz: positive(frequency_hz, defaults.frequency_hz),
            auto_estimate_mutual_inductance: self.auto_estimate_mutual_inductance,
            transferred_power_w: positive(self.transferred_power_w, defaults.transferred_power_w),
            filter_inductance_h: positive(self.filter_inductance_h, defaults.filter_inductance_h),
            mutual_inductance_h: positive(self.mutual_inductance_h, defaults.mutual_inductance_h),
            coil_resistance_ohm: non_negative(
                self.coil_resistance_ohm,
                defaults.coil_resistance_ohm,
            ),
            filter_resistance_ohm: non_negative(
                self.filter_resistance_ohm,
                defaults.filter_resistance_ohm,
            ),
            parallel_cap_resistance_ohm: non_negative(
                self.parallel_cap_resistance_ohm,
                defaults.parallel_cap_resistance_ohm,
            ),
            series_cap_resistance_ohm: non_negative(
                self.series_cap_resistance_ohm,
                defaults.series_cap_resistance_ohm,
            ),
            mosfet_rdson_ohm: non_negative(self.mosfet_rdson_ohm, defaults.mosfet_rdson_ohm),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SolverOptions {
    pub sample_count: usize,
}

impl Default for SolverOptions {
    fn default() -> Self {
        Self { sample_count: 241 }
    }
}

impl SolverOptions {
    fn normalized(&self) -> Self {
        Self {
            sample_count: self.sample_count.clamp(91, 721),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EddyCoefficients {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AngleSolution {
    pub theta_rad: f64,
    pub theta_deg: f64,
    pub residual: f64,
    pub converged: bool,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LossBreakdown {
    pub theta_deg: f64,
    pub transferred_power_w: f64,
    pub required_ac_voltage_rms_v: f64,
    pub input_power_w: f64,
    pub total_loss_w: f64,
    pub efficiency_pct: f64,
    pub coil_filter_loss_w: f64,
    pub capacitor_loss_w: f64,
    pub eddy_loss_w: f64,
    pub mosfet_loss_w: f64,
    pub coil_filter_loss_pct: f64,
    pub capacitor_loss_pct: f64,
    pub eddy_loss_pct: f64,
    pub mosfet_loss_pct: f64,
    pub coil_filter_input_pct: f64,
    pub capacitor_input_pct: f64,
    pub eddy_input_pct: f64,
    pub mosfet_input_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AngleSample {
    pub theta_deg: f64,
    pub efficiency_pct: f64,
    pub total_loss_w: f64,
    pub eddy_loss_pct: f64,
    pub residual: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timings {
    pub eddy: f64,
    pub angle: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericalGrid {
    pub n_rho: usize,
    pub n_z: usize,
    pub n_lambda: usize,
    pub lambda_max: f64,
    pub lambda_step: f64,
    pub auto_lambda_grid: bool,
    pub recommended_n_lambda: usize,
    pub recommended_lambda_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculationResult {
    pub coefficients: EddyCoefficients,
    pub optimum: AngleSolution,
    pub optimum_loss: LossBreakdown,
    pub at_90: LossBreakdown,
    pub samples: Vec<AngleSample>,
    pub numerical_grid: NumericalGrid,
    pub timings_ms: Timings,
    pub estimated_mutual_inductance_h: f64,
    pub used_mutual_inductance_h: f64,
    pub notes: Vec<String>,
}

struct LambdaGridRecommendation {
    n_lambda: usize,
    lambda_max: f64,
}

fn recommend_lambda_grid(params: &EddyParams) -> LambdaGridRecommendation {
    let turns = params.turns.max(1) as f64;
    let mean_radius = (params.coil_radius_m - params.turn_spacing_m * (turns - 1.0) / 2.0)
        .max(params.coil_radius_m * 0.25)
        .max(1.0e-6);
    let min_feature = params.turn_spacing_m.max(1.0e-6);
    let omega = 2.0 * PI * params.frequency_hz;
    let skin_depth = (2.0 / (omega * MU0 * params.conductivity_s_per_m)).sqrt();

    let feature_cutoff = 10.0 / min_feature;
    let skin_cutoff = 8.0 / skin_depth;
    let lambda_max = round_up_to_step(
        DEFAULT_LAMBDA_MAX
            .max(feature_cutoff)
            .max(skin_cutoff)
            .min(AUTO_LAMBDA_MAX_LIMIT),
        10.0,
    );

    let oscillation_length = params.seawater_radius_m + mean_radius;
    let n_lambda = ((lambda_max * oscillation_length * LAMBDA_GRID_SAFETY / PI).ceil() as usize)
        .saturating_add(1)
        .clamp(MIN_RECOMMENDED_N_LAMBDA, MAX_RECOMMENDED_N_LAMBDA);

    LambdaGridRecommendation {
        n_lambda,
        lambda_max,
    }
}

fn compute_eddy_coefficients(params: &EddyParams) -> Result<EddyCoefficients, String> {
    let omega = 2.0 * PI * params.frequency_hz;
    let epsilon = params.relative_permittivity * EPS0;
    let rho = linspace(0.0, params.seawater_radius_m, params.n_rho);
    let rho_w = trapz_weights(&rho);
    let lambda = linspace(0.0, params.lambda_max, params.n_lambda);
    let lambda_w = trapz_weights(&lambda);

    let turns = params.turns as f64;
    let turn_radius_sum =
        turns * params.coil_radius_m - params.turn_spacing_m * turns * (turns - 1.0) / 2.0;
    let mean_radius = params.coil_radius_m - params.turn_spacing_m * (turns - 1.0) / 2.0;
    if mean_radius <= 0.0 {
        return Err("mean coil radius is not positive; reduce turn spacing or turns".to_string());
    }
    if turn_radius_sum <= 0.0 {
        return Err("effective turn radius sum is not positive".to_string());
    }

    let mut u_re = Vec::with_capacity(params.n_lambda);
    let mut u_im = Vec::with_capacity(params.n_lambda);
    let mut kernel_re = Vec::with_capacity(params.n_lambda);
    let mut kernel_im = Vec::with_capacity(params.n_lambda);
    let im_part = omega * MU0 * params.conductivity_s_per_m;
    let omega2_mu_eps = omega * omega * MU0 * epsilon;

    for (idx, &lam) in lambda.iter().enumerate() {
        let (ur, ui) = complex_sqrt(lam * lam - omega2_mu_eps, im_part);
        u_re.push(ur);
        u_im.push(ui);
        let denom = ur * ur + ui * ui;
        let (div_re, div_im) = if denom == 0.0 {
            (0.0, 0.0)
        } else {
            (lam * ur / denom, -lam * ui / denom)
        };
        let shape = lambda_w[idx] * libm::j1(lam * mean_radius);
        kernel_re.push(shape * div_re);
        kernel_im.push(shape * div_im);
    }
    kernel_re[0] = 0.0;
    kernel_im[0] = 0.0;

    let jrho = Array2::from_shape_fn((params.n_rho, params.n_lambda), |(i, k)| {
        libm::j1(rho[i] * lambda[k])
    });
    let kernel_re_arr = Array1::from_vec(kernel_re);
    let kernel_im_arr = Array1::from_vec(kernel_im);
    let source_re = &jrho * &kernel_re_arr.insert_axis(Axis(0));
    let source_im = &jrho * &kernel_im_arr.insert_axis(Axis(0));
    let common = FieldCommon {
        params,
        omega,
        rho: &rho,
        rho_w: &rho_w,
        u_re: &u_re,
        u_im: &u_im,
        source_re,
        source_im,
        turn_radius_sum,
    };

    let middle_z = linspace(0.0, params.coil_gap_m, params.n_z);
    let (a, b) = region_coefficients(&common, &middle_z);
    let side_z = linspace(-params.side_height_m, 0.0, params.n_z);
    let (c, d) = region_coefficients(&common, &side_z);

    Ok(EddyCoefficients { a, b, c, d })
}

fn estimate_mutual_inductance(params: &EddyParams) -> Result<f64, String> {
    let turns = params.turns as f64;
    let outer_radius = params.coil_radius_m;
    let inner_radius = outer_radius - params.turn_spacing_m * (turns - 1.0);
    if inner_radius <= 0.0 {
        return Err("inner coil radius is not positive; reduce turn spacing or turns".to_string());
    }

    let lambda = linspace(0.0, params.lambda_max, params.n_lambda);
    let lambda_w = trapz_weights(&lambda);
    let z = params.coil_gap_m.abs();

    if params.turns == 1 {
        let radius = outer_radius;
        let integral = lambda
            .iter()
            .zip(lambda_w.iter())
            .map(|(&lam, &w)| w * libm::j1(lam * radius).powi(2) * (-lam * z).exp())
            .sum::<f64>();
        return Ok(MU0 * PI * radius * radius * integral);
    }

    let radius_count = params.n_rho.max(24);
    let radii = linspace(inner_radius, outer_radius, radius_count);
    let radius_w = trapz_weights(&radii);
    let radial_width = outer_radius - inner_radius;
    if radial_width <= 0.0 {
        return Err("radial winding width is not positive".to_string());
    }

    let mut integral = 0.0;
    for (&lam, &lam_w) in lambda.iter().zip(lambda_w.iter()) {
        let radial = radii
            .iter()
            .zip(radius_w.iter())
            .map(|(&r, &w)| w * r * libm::j1(lam * r))
            .sum::<f64>();
        integral += lam_w * (-lam * z).exp() * radial * radial;
    }

    Ok(MU0 * PI * turns * turns * integral / (radial_width * radial_width))
}

struct FieldCommon<'a> {
    params: &'a EddyParams,
    omega: f64,
    rho: &'a [f64],
    rho_w: &'a [f64],
    u_re: &'a [f64],
    u_im: &'a [f64],
    source_re: Array2<f64>,
    source_im: Array2<f64>,
    turn_radius_sum: f64,
}

fn region_coefficients(common: &FieldCommon<'_>, z: &[f64]) -> (f64, f64) {
    let z_w = trapz_weights(z);
    let dist_p: Vec<f64> = z.iter().map(|value| value.abs()).collect();
    let dist_s: Vec<f64> = z
        .iter()
        .map(|value| (common.params.coil_gap_m - value).abs())
        .collect();

    let ep_abs = field_abs(common, &dist_p);
    let es_abs = field_abs(common, &dist_s);

    let mut self_integral = 0.0;
    let mut cross_integral = 0.0;
    for i in 0..common.params.n_rho {
        let rho_weight = common.rho_w[i] * common.rho[i];
        for j in 0..common.params.n_z {
            let weight = rho_weight * z_w[j];
            let ep = ep_abs[(i, j)];
            let es = es_abs[(i, j)];
            self_integral += (ep * ep + es * es) * weight;
            cross_integral += (2.0 * ep * es) * weight;
        }
    }

    // The loss model uses P = integral sigma |E|^2 dV, and E has a 1/2 factor.
    let factor =
        2.0 * PI * common.omega * common.omega * MU0 * MU0 * common.params.conductivity_s_per_m
            / 4.0;
    (self_integral * factor, cross_integral * factor)
}

fn field_abs(common: &FieldCommon<'_>, distances: &[f64]) -> Array2<f64> {
    let n_lambda = common.params.n_lambda;
    let n_z = distances.len();

    let exp_re = Array2::from_shape_fn((n_lambda, n_z), |(k, j)| {
        let decay = (-common.u_re[k] * distances[j]).exp();
        let phase = -common.u_im[k] * distances[j];
        decay * phase.cos()
    });
    let exp_im = Array2::from_shape_fn((n_lambda, n_z), |(k, j)| {
        let decay = (-common.u_re[k] * distances[j]).exp();
        let phase = -common.u_im[k] * distances[j];
        decay * phase.sin()
    });

    let real = common.source_re.dot(&exp_re) - common.source_im.dot(&exp_im);
    let imag = common.source_re.dot(&exp_im) + common.source_im.dot(&exp_re);
    let mut abs = Array2::<f64>::zeros(real.dim());
    Zip::from(&mut abs)
        .and(&real)
        .and(&imag)
        .for_each(|out, &re, &im| {
            *out = common.turn_radius_sum * re.hypot(im);
        });
    abs
}

fn find_optimal_angle(circuit: &CircuitParams, coeffs: &EddyCoefficients) -> AngleSolution {
    let lo = PI / 2.0 + 1.0e-8;
    let hi = PI - 1.0e-8;
    let scan_count = 900usize;

    let mut prev_theta = lo;
    let mut prev_value = stationarity(prev_theta, circuit, coeffs);
    let mut best_theta = prev_theta;
    let mut best_abs = prev_value.abs();
    let mut bracket = None;

    for step in 1..=scan_count {
        let theta = lo + (hi - lo) * (step as f64) / (scan_count as f64);
        let value = stationarity(theta, circuit, coeffs);
        if value.abs() < best_abs {
            best_abs = value.abs();
            best_theta = theta;
        }
        if value.is_finite() && prev_value.is_finite() && value * prev_value <= 0.0 {
            bracket = Some((prev_theta, theta));
            break;
        }
        prev_theta = theta;
        prev_value = value;
    }

    let (theta_rad, converged, method) = if let Some((a, b)) = bracket {
        (
            bisect_root(a, b, |theta| stationarity(theta, circuit, coeffs)),
            true,
            "bracketed-bisection",
        )
    } else {
        let theta = golden_section_min(lo, hi, |value| stationarity(value, circuit, coeffs).abs());
        let theta = if theta.is_finite() { theta } else { best_theta };
        (theta, false, "minimum-residual-search")
    };

    AngleSolution {
        theta_rad,
        theta_deg: theta_rad.to_degrees(),
        residual: stationarity(theta_rad, circuit, coeffs),
        converged,
        method: method.to_string(),
    }
}

fn stationarity(theta: f64, c: &CircuitParams, e: &EddyCoefficients) -> f64 {
    let omega = 2.0 * PI * c.frequency_hz;
    let lf = c.filter_inductance_h;
    let r = c.coil_resistance_ohm;
    let m = c.mutual_inductance_h;
    let rf = c.filter_resistance_ohm;
    let rcf = c.parallel_cap_resistance_ohm;
    let rc = c.series_cap_resistance_ohm;
    let rdson = c.mosfet_rdson_ohm;
    let s = theta.sin();
    let co = theta.cos();

    (2.0 * (lf * lf * r + m * m * rf) * co) / (m * s * s * omega * lf * lf)
        + 4.0 * rcf / (lf * omega)
        + (2.0 * (2.0 * lf * m * co * rcf + (rcf + rc) * lf * lf + m * m * rcf) * co)
            / (m * s * s * omega * lf * lf)
        + (4.0 * m * rdson * co) / (s * s * omega * lf * lf)
        - (-e.b * s - 2.0 * e.d * s) / (m * s * omega)
        + ((e.a + e.b * co + 2.0 * e.c + 2.0 * e.d * co) * co) / (m * s * s * omega)
}

fn loss_breakdown(theta: f64, c: &CircuitParams, e: &EddyCoefficients) -> LossBreakdown {
    let omega = 2.0 * PI * c.frequency_hz;
    let lf = c.filter_inductance_h;
    let m = c.mutual_inductance_h;
    let sin_theta = theta.sin().max(1.0e-12);
    let cos_theta = theta.cos();
    let p_trans = c.transferred_power_w;
    let u2 = p_trans * omega * lf * lf / (m * sin_theta);
    let u_rms = u2.sqrt();
    let denom = omega * omega * lf.powi(4);

    let coil_filter_loss =
        2.0 * u2 * (lf * lf * c.coil_resistance_ohm + m * m * c.filter_resistance_ohm) / denom;

    let capacitor_numerator = (m * m + lf * lf + 2.0 * m * lf * cos_theta)
        * c.parallel_cap_resistance_ohm
        + c.series_cap_resistance_ohm * lf * lf;
    let capacitor_loss = 2.0 * u2 * capacitor_numerator / denom;

    let eddy_index = e.a + e.b * cos_theta + 2.0 * e.c + 2.0 * e.d * cos_theta;
    let eddy_loss = u2 * eddy_index / (lf * lf * omega * omega);

    let mosfet_loss = 4.0 * m * m * u2 * c.mosfet_rdson_ohm / denom;

    let total_loss = coil_filter_loss + capacitor_loss + eddy_loss + mosfet_loss;
    let input_power = p_trans + total_loss;
    let efficiency_pct = if input_power > 0.0 {
        100.0 * p_trans / input_power
    } else {
        0.0
    };

    LossBreakdown {
        theta_deg: theta.to_degrees(),
        transferred_power_w: p_trans,
        required_ac_voltage_rms_v: u_rms,
        input_power_w: input_power,
        total_loss_w: total_loss,
        efficiency_pct,
        coil_filter_loss_w: coil_filter_loss,
        capacitor_loss_w: capacitor_loss,
        eddy_loss_w: eddy_loss,
        mosfet_loss_w: mosfet_loss,
        coil_filter_loss_pct: pct(coil_filter_loss, total_loss),
        capacitor_loss_pct: pct(capacitor_loss, total_loss),
        eddy_loss_pct: pct(eddy_loss, total_loss),
        mosfet_loss_pct: pct(mosfet_loss, total_loss),
        coil_filter_input_pct: pct(coil_filter_loss, input_power),
        capacitor_input_pct: pct(capacitor_loss, input_power),
        eddy_input_pct: pct(eddy_loss, input_power),
        mosfet_input_pct: pct(mosfet_loss, input_power),
    }
}

fn build_samples(
    circuit: &CircuitParams,
    coeffs: &EddyCoefficients,
    sample_count: usize,
) -> Vec<AngleSample> {
    let n = sample_count.max(2);
    (0..n)
        .map(|idx| {
            let theta = PI / 2.0 + (PI / 2.0) * (idx as f64) / ((n - 1) as f64);
            let loss = loss_breakdown(theta, circuit, coeffs);
            AngleSample {
                theta_deg: loss.theta_deg,
                efficiency_pct: loss.efficiency_pct,
                total_loss_w: loss.total_loss_w,
                eddy_loss_pct: loss.eddy_loss_pct,
                residual: stationarity(theta, circuit, coeffs),
            }
        })
        .collect()
}

fn bisect_root<F>(mut a: f64, mut b: f64, f: F) -> f64
where
    F: Fn(f64) -> f64,
{
    let mut fa = f(a);
    for _ in 0..90 {
        let mid = 0.5 * (a + b);
        let fm = f(mid);
        if fa * fm <= 0.0 {
            b = mid;
        } else {
            a = mid;
            fa = fm;
        }
    }
    0.5 * (a + b)
}

fn golden_section_min<F>(mut a: f64, mut b: f64, f: F) -> f64
where
    F: Fn(f64) -> f64,
{
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    let mut fc = f(c);
    let mut fd = f(d);
    for _ in 0..120 {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - gr * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + gr * (b - a);
            fd = f(d);
        }
    }
    0.5 * (a + b)
}

fn linspace(start: f64, stop: f64, count: usize) -> Vec<f64> {
    if count == 1 {
        return vec![start];
    }
    let step = (stop - start) / ((count - 1) as f64);
    (0..count).map(|idx| start + step * (idx as f64)).collect()
}

fn trapz_weights(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    let mut weights = vec![0.0; n];
    if n < 2 {
        return weights;
    }
    weights[0] = (values[1] - values[0]) / 2.0;
    weights[n - 1] = (values[n - 1] - values[n - 2]) / 2.0;
    for idx in 1..(n - 1) {
        weights[idx] = (values[idx + 1] - values[idx - 1]) / 2.0;
    }
    weights
}

fn complex_sqrt(re: f64, im: f64) -> (f64, f64) {
    let radius = re.hypot(im);
    let out_re = ((radius + re) / 2.0).max(0.0).sqrt();
    let sign = if im < 0.0 { -1.0 } else { 1.0 };
    let out_im = sign * ((radius - re) / 2.0).max(0.0).sqrt();
    (out_re, out_im)
}

fn positive(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn non_negative(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        fallback
    }
}

fn pct(part: f64, total: f64) -> f64 {
    if total > 0.0 && part.is_finite() {
        100.0 * part / total
    } else {
        0.0
    }
}

fn lambda_step(lambda_max: f64, n_lambda: usize) -> f64 {
    if n_lambda <= 1 {
        0.0
    } else {
        lambda_max / ((n_lambda - 1) as f64)
    }
}

fn round_up_to_step(value: f64, step: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 || step <= 0.0 {
        return DEFAULT_LAMBDA_MAX;
    }
    (value / step).ceil() * step
}

#[cfg(target_arch = "wasm32")]
fn js_now() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn js_now() -> f64 {
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_default_case() {
        let input = ModelInput::default();
        let result = calculate_model(input).expect("calculation should succeed");
        assert!(result.coefficients.a > 0.0);
        assert!(result.coefficients.d > 0.0);
        assert!(result.optimum.theta_deg > 90.0);
        assert!(result.optimum.theta_deg < 180.0);
        assert!(result.optimum_loss.efficiency_pct > 0.0);
        assert_eq!(result.numerical_grid.n_lambda, 640);
        assert!(result.estimated_mutual_inductance_h > 10.0e-6);
        assert!(result.estimated_mutual_inductance_h < 15.0e-6);
    }
}
