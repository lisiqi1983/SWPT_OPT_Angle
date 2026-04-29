# Parameter Reference

## Operating Point

- `frequencyHz`: common operating frequency for the eddy-current model and LCC
  network.
- `transferredPowerW`: target transferred output power used to scale losses.

## Seawater And Coil Geometry

- `conductivitySPerM`: seawater conductivity.
- `relativePermittivity`: relative permittivity used in the complex propagation
  term.
- `coilRadiusM`: outer radius of the planar spiral coil.
- `turns`: number of turns.
- `turnSpacingM`: radial spacing between adjacent turns.
- `coilGapM`: distance between the two coils. The middle seawater region uses
  this value directly.
- `seawaterRadiusM`: radial integration limit of the seawater domain.
- `sideHeightM`: height of one external side region.

## LCC Network And Devices

- `filterInductanceH`: LCC filter inductance `Lf`.
- `mutualInductanceH`: manually provided mutual inductance. It is used only
  when automatic estimation is disabled.
- `autoEstimateMutualInductance`: estimates `M` from coil radius, turn count,
  turn spacing, and coil gap.
- `coilResistanceOhm`: coil resistance term.
- `filterResistanceOhm`: filter inductor ESR.
- `parallelCapResistanceOhm`: compensation capacitor ESR for `Cf`.
- `seriesCapResistanceOhm`: series compensation capacitor ESR.
- `mosfetRdsonOhm`: MOSFET on-resistance used by the conduction loss model.

## Numerical Grid

- `nRho`: radial sample count.
- `nZ`: axial sample count for each integrated region.
- `autoLambdaGrid`: automatically recommends `nLambda` and `lambdaMax` from
  the coil and integration-domain parameters.
- `nLambda`: sample count for the Hankel integral.
- `lambdaMax`: upper integration limit for the fixed lambda grid.

Higher grid counts improve convergence but increase browser runtime.
