//! Deterministic colour maths for the contracted pipeline paths.
//!
//! Everything in this module sticks to basic IEEE float ops (`+ - * /`,
//! comparisons, and bit-pattern moves) per `decisions/determinism-contract.md`:
//!
//! - sRGB→linear is a **const 256-entry lookup table** ([`SRGB_TO_LINEAR`]),
//!   committed as literals — never computed with `powf` at runtime.
//! - Cube roots use the **hand-rolled** [`cbrt_det`] (bit-pattern seed +
//!   Newton iterations), never `f32::cbrt`.
//! - OKLab ([`linear_to_oklab`]) is two matrix multiplies around
//!   [`cbrt_det`].
//!
//! The test module validates the LUT and `cbrt_det` against the runtime
//! formulas (`powf`/`cbrt` are fine **in tests only** — tests don't ship in
//! contracted paths).

/// sRGB 8-bit code value → linear-light intensity (0.0–1.0).
///
/// Committed literals of the IEC 61966-2-1 sRGB EOTF:
/// `c/12.92` for `c <= 0.04045`, else `((c + 0.055)/1.055)^2.4`, with
/// `c = code/255`. Generated once offline at f64 precision and rounded to
/// f32; the agreement test below pins each entry to the formula within
/// 1e-6.
// Literals carry 10 significant digits so each rounds to exactly the
// intended f32 bit pattern; trimming them would move the goldens.
#[allow(clippy::excessive_precision)]
pub const SRGB_TO_LINEAR: [f32; 256] = [
    0.000000000e+00,
    3.035269910e-04,
    6.070539821e-04,
    9.105809731e-04,
    1.214107964e-03,
    1.517634955e-03,
    1.821161946e-03,
    2.124688821e-03,
    2.428215928e-03,
    2.731742803e-03,
    3.035269910e-03,
    3.346535843e-03,
    3.676507389e-03,
    4.024717025e-03,
    4.391442053e-03,
    4.776953254e-03,
    5.181516521e-03,
    5.605391692e-03,
    6.048833020e-03,
    6.512090564e-03,
    6.995410193e-03,
    7.499032188e-03,
    8.023193106e-03,
    8.568125777e-03,
    9.134058841e-03,
    9.721217677e-03,
    1.032982301e-02,
    1.096009370e-02,
    1.161224488e-02,
    1.228648797e-02,
    1.298303250e-02,
    1.370208338e-02,
    1.444384363e-02,
    1.520851441e-02,
    1.599629410e-02,
    1.680737548e-02,
    1.764195412e-02,
    1.850022003e-02,
    1.938236132e-02,
    2.028856240e-02,
    2.121900953e-02,
    2.217388526e-02,
    2.315336652e-02,
    2.415763214e-02,
    2.518685907e-02,
    2.624122240e-02,
    2.732089162e-02,
    2.842603996e-02,
    2.955683507e-02,
    3.071344458e-02,
    3.189603239e-02,
    3.310476616e-02,
    3.433980793e-02,
    3.560131416e-02,
    3.688944876e-02,
    3.820437193e-02,
    3.954623640e-02,
    4.091519862e-02,
    4.231141135e-02,
    4.373503104e-02,
    4.518620297e-02,
    4.666508734e-02,
    4.817182571e-02,
    4.970656708e-02,
    5.126945674e-02,
    5.286064744e-02,
    5.448027700e-02,
    5.612849072e-02,
    5.780543014e-02,
    5.951123685e-02,
    6.124605238e-02,
    6.301001459e-02,
    6.480326504e-02,
    6.662593782e-02,
    6.847816706e-02,
    7.036009431e-02,
    7.227185369e-02,
    7.421357185e-02,
    7.618538290e-02,
    7.818742096e-02,
    8.021982014e-02,
    8.228270710e-02,
    8.437620848e-02,
    8.650045842e-02,
    8.865558356e-02,
    9.084171057e-02,
    9.305896610e-02,
    9.530746937e-02,
    9.758734703e-02,
    9.989872575e-02,
    1.022417322e-01,
    1.046164855e-01,
    1.070231050e-01,
    1.094617099e-01,
    1.119324267e-01,
    1.144353747e-01,
    1.169706658e-01,
    1.195384264e-01,
    1.221387759e-01,
    1.247718185e-01,
    1.274376810e-01,
    1.301364750e-01,
    1.328683197e-01,
    1.356333345e-01,
    1.384316087e-01,
    1.412632912e-01,
    1.441284716e-01,
    1.470272690e-01,
    1.499597877e-01,
    1.529261470e-01,
    1.559264660e-01,
    1.589608341e-01,
    1.620293707e-01,
    1.651321948e-01,
    1.682693958e-01,
    1.714411080e-01,
    1.746474057e-01,
    1.778884232e-01,
    1.811642498e-01,
    1.844749898e-01,
    1.878207773e-01,
    1.912016869e-01,
    1.946178377e-01,
    1.980693191e-01,
    2.015562505e-01,
    2.050787359e-01,
    2.086368650e-01,
    2.122307569e-01,
    2.158605009e-01,
    2.195262015e-01,
    2.232279629e-01,
    2.269658744e-01,
    2.307400554e-01,
    2.345505804e-01,
    2.383975685e-01,
    2.422811240e-01,
    2.462013215e-01,
    2.501582801e-01,
    2.541520894e-01,
    2.581828535e-01,
    2.622506618e-01,
    2.663556039e-01,
    2.704977989e-01,
    2.746773064e-01,
    2.788942754e-01,
    2.831487358e-01,
    2.874408364e-01,
    2.917706370e-01,
    2.961382568e-01,
    3.005437851e-01,
    3.049873114e-01,
    3.094689250e-01,
    3.139887154e-01,
    3.185467720e-01,
    3.231432140e-01,
    3.277781010e-01,
    3.324515224e-01,
    3.371636271e-01,
    3.419144154e-01,
    3.467040658e-01,
    3.515326083e-01,
    3.564001322e-01,
    3.613067865e-01,
    3.662526011e-01,
    3.712376952e-01,
    3.762621284e-01,
    3.813260198e-01,
    3.864294291e-01,
    3.915724754e-01,
    3.967552185e-01,
    4.019777775e-01,
    4.072402120e-01,
    4.125426114e-01,
    4.178850651e-01,
    4.232676625e-01,
    4.286904931e-01,
    4.341536462e-01,
    4.396571815e-01,
    4.452011883e-01,
    4.507857859e-01,
    4.564110339e-01,
    4.620769918e-01,
    4.677838087e-01,
    4.735314846e-01,
    4.793201685e-01,
    4.851499498e-01,
    4.910208583e-01,
    4.969329834e-01,
    5.028864741e-01,
    5.088813305e-01,
    5.149176717e-01,
    5.209955573e-01,
    5.271151066e-01,
    5.332763791e-01,
    5.394794941e-01,
    5.457244515e-01,
    5.520114303e-01,
    5.583403707e-01,
    5.647115111e-01,
    5.711248517e-01,
    5.775804520e-01,
    5.840784311e-01,
    5.906188488e-01,
    5.972017646e-01,
    6.038273573e-01,
    6.104955673e-01,
    6.172065735e-01,
    6.239603758e-01,
    6.307571530e-01,
    6.375968456e-01,
    6.444796920e-01,
    6.514056325e-01,
    6.583748460e-01,
    6.653872728e-01,
    6.724431515e-01,
    6.795424819e-01,
    6.866853237e-01,
    6.938717365e-01,
    7.011018991e-01,
    7.083757520e-01,
    7.156934738e-01,
    7.230551243e-01,
    7.304607630e-01,
    7.379103899e-01,
    7.454041839e-01,
    7.529422045e-01,
    7.605245113e-01,
    7.681511641e-01,
    7.758222222e-01,
    7.835378051e-01,
    7.912979126e-01,
    7.991027236e-01,
    8.069522381e-01,
    8.148465753e-01,
    8.227857351e-01,
    8.307698965e-01,
    8.387989998e-01,
    8.468732238e-01,
    8.549926281e-01,
    8.631572127e-01,
    8.713670969e-01,
    8.796223998e-01,
    8.879231215e-01,
    8.962693810e-01,
    9.046611786e-01,
    9.130986333e-01,
    9.215818644e-01,
    9.301108718e-01,
    9.386857152e-01,
    9.473065138e-01,
    9.559733272e-01,
    9.646862745e-01,
    9.734452963e-01,
    9.822505713e-01,
    9.911020994e-01,
    1.000000000e+00,
];

/// Decode one 8-bit sRGB triple to linear light through the LUT.
#[must_use]
pub fn srgb8_to_linear(rgb: [u8; 3]) -> [f32; 3] {
    [
        SRGB_TO_LINEAR[usize::from(rgb[0])],
        SRGB_TO_LINEAR[usize::from(rgb[1])],
        SRGB_TO_LINEAR[usize::from(rgb[2])],
    ]
}

/// Deterministic cube root for non-negative inputs.
///
/// Seed from the classic exponent-division bit hack
/// (`bits / 3 + 0x2a51_4067`), then four Newton iterations
/// `y ← (2y + x/y²) / 3` — multiplications, divisions, and additions only,
/// all bit-deterministic IEEE ops, so the result is identical on every
/// platform (unlike libm's `f32::cbrt`). Inputs ≤ 0 return 0.0: the only
/// callers feed it LMS components, which are non-negative combinations of
/// linear RGB.
///
/// Accuracy: the seed is within ~5% and Newton converges quadratically, so
/// four iterations land well inside 1e-6 of the true cube root over the
/// 0..=1 working domain (pinned by the sweep test below).
#[must_use]
pub fn cbrt_det(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut y = f32::from_bits(x.to_bits() / 3 + 0x2a51_4067);
    let mut i = 0;
    while i < 4 {
        y = (2.0 * y + x / (y * y)) / 3.0;
        i += 1;
    }
    y
}

/// Linear-light sRGB → OKLab.
///
/// Matrices from Björn Ottosson's published OKLab definition:
/// <https://bottosson.github.io/posts/oklab/> ("Converting from linear
/// sRGB to Oklab"). Matrix multiplies + [`cbrt_det`] only — no libm.
#[must_use]
pub fn linear_to_oklab(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb;

    let l = 0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;

    let l_ = cbrt_det(l);
    let m_ = cbrt_det(m);
    let s_ = cbrt_det(s);

    [
        0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
        1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
        0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
    ]
}

/// Linear-light sRGB → Y′UV-shaped luma/chroma coordinates.
///
/// BT.601 coefficients (Y = 0.299 R + 0.587 G + 0.114 B and the standard
/// U/V differences). Classically these apply to gamma-encoded R′G′B′; this
/// pipeline applies them to **linear** RGB so every metric shares one input
/// space and stays inside basic-ops territory — documented simplification.
#[must_use]
pub fn linear_to_yuv(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb;
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let u = -0.147_13 * r - 0.288_86 * g + 0.436 * b;
    let v = 0.615 * r - 0.514_99 * g - 0.100_01 * b;
    [y, u, v]
}

/// The colour-distance metric used for quantisation and constraint search.
///
/// Each variant is a squared-distance function over a projection of linear
/// RGB; all are basic-ops only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Metric {
    /// Distances in OKLab (perceptually uniform; the default).
    #[default]
    OkLab,
    /// Weighted squared distance in linear RGB: `3Δr² + 4Δg² + 2Δb²`.
    WeightedRgb,
    /// Euclidean squared distance in BT.601 YUV computed from linear RGB.
    Yuv,
}

impl Metric {
    /// Project a linear-RGB triple into this metric's coordinate space.
    #[must_use]
    pub fn project(self, linear: [f32; 3]) -> [f32; 3] {
        match self {
            Self::OkLab => linear_to_oklab(linear),
            Self::WeightedRgb => linear,
            Self::Yuv => linear_to_yuv(linear),
        }
    }

    /// Squared distance between two projected coordinates.
    #[must_use]
    pub fn distance_sq(self, a: [f32; 3], b: [f32; 3]) -> f32 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        match self {
            Self::WeightedRgb => 3.0 * dx * dx + 4.0 * dy * dy + 2.0 * dz * dz,
            Self::OkLab | Self::Yuv => dx * dx + dy * dy + dz * dz,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Runtime powf/cbrt are allowed here: tests validate the deterministic
    // tables/functions against the reference formulas but never ship in
    // contracted paths.

    fn srgb_formula(code: u8) -> f64 {
        let c = f64::from(code) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    #[test]
    fn lut_endpoints_are_exact() {
        assert_eq!(SRGB_TO_LINEAR[0], 0.0);
        assert_eq!(SRGB_TO_LINEAR[255], 1.0);
    }

    #[test]
    fn lut_is_strictly_monotonic() {
        for pair in SRGB_TO_LINEAR.windows(2) {
            assert!(pair[0] < pair[1], "LUT not strictly increasing at {pair:?}");
        }
    }

    #[test]
    fn lut_agrees_with_runtime_formula() {
        for (code, &entry) in SRGB_TO_LINEAR.iter().enumerate() {
            let reference = srgb_formula(u8::try_from(code).expect("code fits u8"));
            assert!(
                (f64::from(entry) - reference).abs() < 1e-6,
                "LUT[{code}] = {entry} vs formula {reference}"
            );
        }
    }

    #[test]
    fn lut_srgb_128_matches_known_value() {
        // sRGB 128 decodes to ~0.2158 linear — the canonical "mid grey is
        // not 0.5" check.
        assert!((SRGB_TO_LINEAR[128] - 0.215_860_5).abs() < 1e-6);
    }

    #[test]
    fn cbrt_det_matches_std_cbrt_over_sweep() {
        // Sweep the working domain plus a margin above 1.0.
        let mut x = 0.0f32;
        while x <= 2.0 {
            let ours = cbrt_det(x);
            let std = x.cbrt();
            assert!(
                (ours - std).abs() < 1e-6,
                "cbrt_det({x}) = {ours} vs std {std}"
            );
            x += 1e-4;
        }
    }

    #[test]
    fn cbrt_det_zero_and_negative_clamp() {
        assert_eq!(cbrt_det(0.0), 0.0);
        assert_eq!(cbrt_det(-1.0), 0.0);
    }

    #[test]
    fn oklab_white_is_unit_lightness() {
        // Linear white (1,1,1) must land at L ≈ 1, a ≈ 0, b ≈ 0.
        let [l, a, b] = linear_to_oklab([1.0, 1.0, 1.0]);
        assert!((l - 1.0).abs() < 1e-3, "L = {l}");
        assert!(a.abs() < 1e-3, "a = {a}");
        assert!(b.abs() < 1e-3, "b = {b}");
    }
}
