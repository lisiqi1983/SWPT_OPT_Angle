import init, { calculate } from "../pkg/swpt_core.js";

const statusEl = document.getElementById("status");
const runButton = document.getElementById("runButton");
let wasmReady = false;
let lastResult = null;

const inputIds = [
  "frequencyKHz", "powerW", "conductivity", "epsilonR", "coilRadius", "turns",
  "turnSpacingMm", "coilGap", "seaRadius", "sideHeight",
  "lfUh", "mUh", "autoM", "coilR", "filterR", "rcf", "rc", "rdson",
  "nRho", "nZ", "autoLambdaGrid", "nLambda", "lambdaMax"
];

const fieldHints = {
  frequencyKHz: "Operating frequency shared by the seawater eddy-current model and the LCC loss equations.",
  powerW: "Fixed transferred output power used to scale voltage, current, and all loss terms.",
  conductivity: "Seawater conductivity. Larger values increase eddy-current loss and reduce skin depth.",
  epsilonR: "Relative permittivity of seawater used in the complex propagation term.",
  coilRadius: "Outer radius of the planar spiral coil. It affects the field kernel and the mutual-inductance estimate.",
  turns: "Number of turns in each symmetric coil.",
  turnSpacingMm: "Radial spacing between adjacent turns. It affects mean coil radius, M estimation, and lambda-grid recommendation.",
  coilGap: "Distance between the two coils. The middle seawater region height is equal to this value.",
  seaRadius: "Radial integration limit for the seawater domain. Large values require a denser lambda grid.",
  sideHeight: "Height of one external side region. The model counts two symmetric side regions in the final eddy coefficient.",
  lfUh: "LCC filter inductance Lf in microhenry.",
  mUh: "Manual mutual inductance in microhenry. It is used only when Mutual Inductance Mode is set to manual.",
  coilR: "Coil resistance term used in the coil/filter loss expression.",
  filterR: "Equivalent series resistance of the LCC filter inductor.",
  rcf: "Equivalent series resistance of the parallel compensation capacitor Cf.",
  rc: "Equivalent series resistance of the series compensation capacitor.",
  rdson: "MOSFET on-resistance used by the conduction-loss term.",
  nRho: "Number of radial samples in the seawater integral. Higher values improve spatial convergence.",
  nZ: "Number of axial samples for each integrated region. Higher values improve spatial convergence.",
  nLambda: "Number of samples in the Hankel lambda integral. Auto mode increases this for large integration radii.",
  lambdaMax: "Upper limit of the lambda integral. Auto mode recommends this from turn spacing and skin depth."
};

function value(id) {
  const raw = Number(document.getElementById(id).value);
  if (!Number.isFinite(raw)) {
    throw new Error(`Invalid parameter: ${id}`);
  }
  return raw;
}

function inputPayload() {
  const frequencyHz = value("frequencyKHz") * 1e3;
  return {
    eddy: {
      frequencyHz,
      conductivitySPerM: value("conductivity"),
      relativePermittivity: value("epsilonR"),
      coilRadiusM: value("coilRadius"),
      turns: value("turns"),
      turnSpacingM: value("turnSpacingMm") * 1e-3,
      coilGapM: value("coilGap"),
      seawaterRadiusM: value("seaRadius"),
      sideHeightM: value("sideHeight"),
      nRho: value("nRho"),
      nZ: value("nZ"),
      autoLambdaGrid: document.getElementById("autoLambdaGrid").checked,
      nLambda: value("nLambda"),
      lambdaMax: value("lambdaMax")
    },
    circuit: {
      frequencyHz,
      autoEstimateMutualInductance: document.getElementById("autoM").checked,
      transferredPowerW: value("powerW"),
      filterInductanceH: value("lfUh") * 1e-6,
      mutualInductanceH: value("mUh") * 1e-6,
      coilResistanceOhm: value("coilR"),
      filterResistanceOhm: value("filterR"),
      parallelCapResistanceOhm: value("rcf"),
      seriesCapResistanceOhm: value("rc"),
      mosfetRdsonOhm: value("rdson")
    },
    options: {
      sampleCount: 241
    }
  };
}

function fmt(value, digits = 3) {
  if (!Number.isFinite(value)) return "--";
  const abs = Math.abs(value);
  if ((abs > 0 && abs < 1e-3) || abs >= 1e5) {
    return value.toExponential(digits);
  }
  return value.toFixed(digits);
}

function pct(value) {
  return `${fmt(value, 2)}%`;
}

function watts(value) {
  return `${fmt(value, 2)} W`;
}

function ms(value) {
  if (value < 1000) return `${fmt(value, 0)} ms`;
  return `${fmt(value / 1000, 2)} s`;
}

function setStatus(text, mode = "") {
  statusEl.textContent = text;
  statusEl.dataset.mode = mode;
}

function enhanceFieldHints() {
  Object.entries(fieldHints).forEach(([id, tip]) => {
    const input = document.getElementById(id);
    const label = input?.closest("label");
    if (!input || !label || label.classList.contains("switch-field")) return;
    if (label.querySelector(".label-line")) return;

    const textNodes = Array.from(label.childNodes)
      .filter((node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim());
    const text = textNodes.map((node) => node.textContent.trim()).join(" ");
    if (!text) return;
    textNodes.forEach((node) => node.remove());

    const line = document.createElement("span");
    line.className = "label-line";
    const title = document.createElement("span");
    title.textContent = text;
    const hint = document.createElement("span");
    hint.className = "hint";
    hint.tabIndex = 0;
    hint.textContent = "?";
    hint.dataset.tip = tip;
    hint.setAttribute("aria-label", `${text}: ${tip}`);
    line.append(title, hint);
    label.insertBefore(line, input);
  });
}

function syncMutualInputState() {
  const auto = document.getElementById("autoM").checked;
  document.getElementById("mUh").disabled = auto;
  document.getElementById("autoMMode").textContent = auto ? "Geometry estimate" : "Manual value";
}

function syncLambdaGridState() {
  const auto = document.getElementById("autoLambdaGrid").checked;
  document.getElementById("nLambda").disabled = auto;
  document.getElementById("lambdaMax").disabled = auto;
  document.getElementById("autoLambdaMode").textContent = auto ? "Recommended grid" : "Manual grid";
}

async function run() {
  if (!wasmReady) return;
  runButton.disabled = true;
  setStatus("Calculating...", "busy");
  await new Promise((resolve) => requestAnimationFrame(resolve));

  try {
    const result = calculate(inputPayload());
    lastResult = result;
    render(result);
    setStatus(`Done: ${ms(result.timingsMs.total)}`, "ok");
  } catch (error) {
    setStatus(error.message || String(error), "error");
  } finally {
    runButton.disabled = false;
  }
}

function render(result) {
  const opt = result.optimum;
  const loss = result.optimumLoss;
  const at90 = result.at90;
  const gain = loss.efficiencyPct - at90.efficiencyPct;

  document.getElementById("thetaValue").textContent = `${fmt(opt.thetaDeg, 3)} deg`;
  document.getElementById("efficiencyValue").textContent = pct(loss.efficiencyPct);
  document.getElementById("lossValue").textContent = watts(loss.totalLossW);
  document.getElementById("timeValue").textContent = ms(result.timingsMs.total);
  document.getElementById("eff90").textContent = pct(at90.efficiencyPct);
  document.getElementById("effOpt").textContent = pct(loss.efficiencyPct);
  document.getElementById("gainValue").textContent = `Δη = ${gain >= 0 ? "+" : ""}${fmt(gain, 2)}%`;
  document.getElementById("voltageValue").textContent = `${fmt(loss.requiredAcVoltageRmsV, 2)} V`;
  document.getElementById("mutualValue").textContent = `${fmt(result.usedMutualInductanceH * 1e6, 3)} uH`;
  document.getElementById("lambdaGridValue").textContent =
    `${result.numericalGrid.nLambda} / ${fmt(result.numericalGrid.lambdaMax, 0)}`;
  if (document.getElementById("autoM").checked) {
    document.getElementById("mUh").value = fmt(result.usedMutualInductanceH * 1e6, 4);
  }
  if (document.getElementById("autoLambdaGrid").checked) {
    document.getElementById("nLambda").value = result.numericalGrid.nLambda;
    document.getElementById("lambdaMax").value = fmt(result.numericalGrid.lambdaMax, 0);
  }
  const mUh = result.usedMutualInductanceH * 1e6;
  const consistencyNote = mUh <= 3
    ? "M is small for the selected target power; absolute efficiency may be low unless M, Lf, resistances, and power come from the same operating case."
    : "Absolute efficiency is meaningful only when M, Lf, resistances, and target power come from the same operating case.";
  document.getElementById("summaryText").textContent =
    `Solver: ${opt.method}. Stationarity residual: ${fmt(opt.residual, 3)}. Lambda step: ${fmt(result.numericalGrid.lambdaStep, 3)}. ${consistencyNote}`;

  renderLossBars(loss);
  renderTables(result);
  drawChart(
    document.getElementById("efficiencyChart"),
    result.samples,
    "efficiencyPct",
    opt.thetaDeg,
    "#2563eb",
    "Efficiency (%)"
  );
  drawChart(
    document.getElementById("eddyChart"),
    result.samples,
    "eddyLossPct",
    opt.thetaDeg,
    "#0f8b5f",
    "Eddy share of total loss (%)"
  );
}

function renderLossBars(loss) {
  const rows = [
    ["Coil / Filter Inductor", loss.coilFilterLossPct, loss.coilFilterLossW, "#2563eb"],
    ["Compensation Capacitors", loss.capacitorLossPct, loss.capacitorLossW, "#7c3aed"],
    ["Seawater Eddy Current", loss.eddyLossPct, loss.eddyLossW, "#0f8b5f"],
    ["MOSFET Conduction", loss.mosfetLossPct, loss.mosfetLossW, "#d23b53"]
  ];
  document.getElementById("lossBars").innerHTML = rows.map(([name, share, amount, color]) => `
    <div class="loss-row">
      <div class="loss-row-top">
        <span>${name}</span>
        <strong>${pct(share)} · ${watts(amount)}</strong>
      </div>
      <div class="track"><i style="width:${Math.max(0, Math.min(100, share))}%; background:${color}"></i></div>
    </div>
  `).join("");
}

function renderTables(result) {
  const c = result.coefficients;
  document.getElementById("coeffRows").innerHTML = [
    ["A", c.a, "Middle-region self term"],
    ["B", c.b, "Middle-region cross term"],
    ["C", c.c, "One-side-region self term"],
    ["D", c.d, "One-side-region cross term"]
  ].map(([name, val, desc]) => `<tr><td>${name}</td><td>${fmt(val, 8)}</td><td>${desc}</td></tr>`).join("");

  const l = result.optimumLoss;
  document.getElementById("lossRows").innerHTML = [
    ["Coil / Filter Inductor", l.coilFilterLossW, l.coilFilterInputPct],
    ["Compensation Capacitors", l.capacitorLossW, l.capacitorInputPct],
    ["Seawater Eddy Current", l.eddyLossW, l.eddyInputPct],
    ["MOSFET Conduction", l.mosfetLossW, l.mosfetInputPct],
    ["Total Loss", l.totalLossW, 100 - l.efficiencyPct]
  ].map(([name, w, p]) => `<tr><td>${name}</td><td>${watts(w)}</td><td>${pct(p)}</td></tr>`).join("");
}

function setupCanvas(canvas) {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(rect.width * dpr);
  canvas.height = Math.round(rect.height * dpr);
  const ctx = canvas.getContext("2d");
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { ctx, width: rect.width, height: rect.height };
}

function drawChart(canvas, samples, key, thetaDeg, color, label) {
  const { ctx, width, height } = setupCanvas(canvas);
  const plot = { left: 48, right: width - 18, top: 26, bottom: height - 34 };
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, width, height);

  const ys = samples.map((sample) => sample[key]);
  const min = Math.min(...ys);
  const max = Math.max(...ys);
  const pad = Math.max((max - min) * 0.08, 0.1);
  const xScale = (x) => plot.left + (x - 90) * (plot.right - plot.left) / 90;
  const yScale = (y) => plot.bottom - (y - min + pad) * (plot.bottom - plot.top) / (max - min + 2 * pad);

  ctx.strokeStyle = "#d8dde6";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(plot.left, plot.top);
  ctx.lineTo(plot.left, plot.bottom);
  ctx.lineTo(plot.right, plot.bottom);
  ctx.stroke();

  ctx.fillStyle = "#687386";
  ctx.font = "11px system-ui, sans-serif";
  ctx.fillText(label, plot.left, 15);
  ctx.fillText("90", plot.left - 3, plot.bottom + 18);
  ctx.fillText("180 deg", plot.right - 42, plot.bottom + 18);

  ctx.strokeStyle = color;
  ctx.lineWidth = 2.5;
  ctx.beginPath();
  samples.forEach((sample, index) => {
    const x = xScale(sample.thetaDeg);
    const y = yScale(sample[key]);
    if (index === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();

  const markerX = xScale(thetaDeg);
  ctx.strokeStyle = "#111827";
  ctx.setLineDash([4, 4]);
  ctx.beginPath();
  ctx.moveTo(markerX, plot.top);
  ctx.lineTo(markerX, plot.bottom);
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.fillStyle = "#111827";
  ctx.font = "600 11px system-ui, sans-serif";
  ctx.fillText(`${fmt(thetaDeg, 2)} deg`, Math.min(markerX + 6, plot.right - 70), plot.top + 14);
}

async function boot() {
  try {
    await init();
    wasmReady = true;
    runButton.disabled = false;
    setStatus("WASM loaded", "ok");
    await run();
  } catch (error) {
    setStatus("WASM failed to load. Run the build script first.", "error");
    console.error(error);
  }
}

runButton.addEventListener("click", run);
inputIds.forEach((id) => {
  document.getElementById(id).addEventListener("keydown", (event) => {
    if (event.key === "Enter") run();
  });
});
document.getElementById("autoM").addEventListener("change", () => {
  syncMutualInputState();
  run();
});
document.getElementById("autoLambdaGrid").addEventListener("change", () => {
  syncLambdaGridState();
  run();
});
window.addEventListener("resize", () => {
  if (!lastResult) return;
  drawChart(document.getElementById("efficiencyChart"), lastResult.samples, "efficiencyPct", lastResult.optimum.thetaDeg, "#2563eb", "Efficiency (%)");
  drawChart(document.getElementById("eddyChart"), lastResult.samples, "eddyLossPct", lastResult.optimum.thetaDeg, "#0f8b5f", "Eddy share of total loss (%)");
});

enhanceFieldHints();
syncMutualInputState();
syncLambdaGridState();
boot();
