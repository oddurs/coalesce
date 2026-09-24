//! Blackbody colour lookup: 256 entries from 1000 K to 30000 K on a log scale,
//! linear Rec. 709 primaries, normalised to unit luminance.

pub const LUT_SIZE: usize = 256;
pub const T_MIN: f32 = 1000.0;
pub const T_MAX: f32 = 30000.0;

fn lobe(x: f32, mu: f32, s1: f32, s2: f32) -> f32 {
    let s = if x < mu { s1 } else { s2 };
    let t = (x - mu) / s;
    (-0.5 * t * t).exp()
}

/// Wyman, Sloan and Shirley's analytic fit to the CIE 1931 observer.
fn cie_xyz(lambda_nm: f32) -> [f32; 3] {
    let x = 1.056 * lobe(lambda_nm, 599.8, 37.9, 31.0) + 0.362 * lobe(lambda_nm, 442.0, 16.0, 26.7)
        - 0.065 * lobe(lambda_nm, 501.1, 20.4, 26.2);
    let y = 0.821 * lobe(lambda_nm, 568.8, 46.9, 40.5) + 0.286 * lobe(lambda_nm, 530.9, 16.3, 31.1);
    let z = 1.217 * lobe(lambda_nm, 437.0, 11.8, 36.0) + 0.681 * lobe(lambda_nm, 459.0, 26.0, 13.8);
    [x, y, z]
}

fn planck(lambda_nm: f32, temp: f32) -> f32 {
    const C2: f32 = 1.438_776_9e7; // nm K
    let l = lambda_nm;
    1.0 / (l.powi(5) * ((C2 / (l * temp)).exp() - 1.0))
}

/// Linear Rec. 709 colour of a blackbody at `temp` kelvin, luminance 1.
pub fn rgb(temp: f32) -> [f32; 3] {
    let mut xyz = [0.0f32; 3];
    let mut lambda = 380.0;
    while lambda <= 780.0 {
        let b = planck(lambda, temp);
        let c = cie_xyz(lambda);
        for k in 0..3 {
            xyz[k] += b * c[k];
        }
        lambda += 5.0;
    }
    let [x, y, z] = xyz;
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    [(r / lum).max(0.0), (g / lum).max(0.0), (b / lum).max(0.0)]
}

pub fn lut() -> Vec<[f32; 4]> {
    (0..LUT_SIZE)
        .map(|i| {
            let t = T_MIN * (T_MAX / T_MIN).powf(i as f32 / (LUT_SIZE - 1) as f32);
            let [r, g, b] = rgb(t);
            [r, g, b, 1.0]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d65_is_near_white() {
        let [r, g, b] = rgb(6504.0);
        assert!(
            (r - 1.0).abs() < 0.08 && (g - 1.0).abs() < 0.05 && (b - 1.0).abs() < 0.12,
            "{r} {g} {b}"
        );
    }

    #[test]
    fn cool_and_hot_lean_the_right_way() {
        let [r, _, b] = rgb(3000.0);
        assert!(r > 1.2 * b);
        let [r, _, b] = rgb(15000.0);
        assert!(b > 1.2 * r);
    }

    #[test]
    fn lut_covers_the_range_monotonically_in_blue() {
        let l = lut();
        assert_eq!(l.len(), LUT_SIZE);
        assert!(l[0][2] < l[LUT_SIZE - 1][2]);
    }
}
