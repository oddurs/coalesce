//! CPU reference for the integrator in `shaders/trace.wgsl`. The shader is the
//! renderer; this module exists so the physics has tests.

pub type V3 = [f32; 3];

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn axpy(a: f32, x: V3, y: V3) -> V3 {
    [a * x[0] + y[0], a * x[1] + y[1], a * x[2] + y[2]]
}

/// Schwarzschild orbit equation in vector form, superposed over bodies given
/// as (position, Schwarzschild radius).
pub fn accel(x: V3, v: V3, bodies: &[(V3, f32)]) -> V3 {
    let mut a = [0.0; 3];
    for &(p, rs) in bodies {
        let r = axpy(-1.0, p, x);
        let r2 = dot(r, r);
        let h = cross(r, v);
        let h2 = dot(h, h);
        let r5 = r2 * r2 * r2.sqrt();
        let k = -1.5 * rs * h2 / r5.max(1e-6);
        a = axpy(k, r, a);
    }
    a
}

#[derive(Debug, PartialEq)]
pub enum Fate {
    Captured,
    Escaped { direction: V3 },
}

/// Integrate a ray with RK4 until it falls through a horizon or leaves the
/// escape radius moving outward.
pub fn trace(
    origin: V3,
    dir: V3,
    bodies: &[(V3, f32)],
    escape_radius: f32,
    max_steps: usize,
) -> Fate {
    let mut x = origin;
    let mut v = dir;
    for _ in 0..max_steps {
        let mut rmin = f32::MAX;
        for &(p, rs) in bodies {
            let r = dot(axpy(-1.0, p, x), axpy(-1.0, p, x)).sqrt();
            if r < rs {
                return Fate::Captured;
            }
            rmin = rmin.min(r);
        }
        if dot(x, x).sqrt() > escape_radius && dot(x, v) > 0.0 {
            let l = dot(v, v).sqrt();
            return Fate::Escaped {
                direction: [v[0] / l, v[1] / l, v[2] / l],
            };
        }
        let h = (0.05 * rmin).clamp(0.01, 0.5) / dot(v, v).sqrt();
        let k1v = accel(x, v, bodies);
        let k1x = v;
        let k2v = accel(axpy(0.5 * h, k1x, x), axpy(0.5 * h, k1v, v), bodies);
        let k2x = axpy(0.5 * h, k1v, v);
        let k3v = accel(axpy(0.5 * h, k2x, x), axpy(0.5 * h, k2v, v), bodies);
        let k3x = axpy(0.5 * h, k2v, v);
        let k4v = accel(axpy(h, k3x, x), axpy(h, k3v, v), bodies);
        let k4x = axpy(h, k3v, v);
        for i in 0..3 {
            x[i] += h / 6.0 * (k1x[i] + 2.0 * k2x[i] + 2.0 * k3x[i] + k4x[i]);
            v[i] += h / 6.0 * (k1v[i] + 2.0 * k2v[i] + 2.0 * k3v[i] + k4v[i]);
        }
    }
    Fate::Captured
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shoot(b: f32) -> Fate {
        let hole = [([0.0, 0.0, 0.0], 1.0)];
        trace([-200.0, b, 0.0], [1.0, 0.0, 0.0], &hole, 400.0, 200_000)
    }

    #[test]
    fn shadow_edge_is_at_the_critical_impact_parameter() {
        // b_crit = 3 sqrt(3) / 2 rs = 2.598 rs.
        assert_eq!(shoot(2.55), Fate::Captured);
        assert!(matches!(shoot(2.65), Fate::Escaped { .. }));
    }

    #[test]
    fn weak_field_deflection_matches_einstein() {
        // alpha = 2 rs / b for b >> rs.
        let b = 60.0;
        let Fate::Escaped { direction } = shoot(b) else {
            panic!("captured")
        };
        let alpha = (-direction[1]).atan2(direction[0]);
        let expected = 2.0 / b;
        assert!(
            (alpha - expected).abs() < 0.05 * expected,
            "alpha {alpha} expected {expected}"
        );
    }

    #[test]
    fn two_bodies_add_up_to_one_when_coincident() {
        let x = [3.0, 1.0, 0.5];
        let v = [0.2, 0.9, -0.1];
        let one = accel(x, v, &[([0.0; 3], 2.0)]);
        let two = accel(x, v, &[([0.0; 3], 1.2), ([0.0; 3], 0.8)]);
        for i in 0..3 {
            assert!((one[i] - two[i]).abs() < 1e-5);
        }
    }
}
