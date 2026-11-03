//! Mouvements et pavé tactile d'une manette (DualSense, DualShock 4), lus
//! par le fil SDL : gyroscope, accéléromètre, orientation calculée, doigts
//! sur le pavé tactile et son clic. Repris d'`orientation.c` de chiaki-ng
//! (algorithme IMU de Madgwick, avec son filtre de petites variations).

/// Gravité standard (`SDL_STANDARD_GRAVITY`), pour l'accéléromètre en g.
const GRAVITY: f32 = 9.80665;
const WARMUP_SAMPLES: u64 = 30;
const BETA_WARMUP: f32 = 20.0;
const BETA_DEFAULT: f32 = 0.05;
const ORIENT_FUZZ: f32 = 0.0007;
const FUZZ_PREV_WEIGHT: f32 = 0.75;
const FUZZ_PREV_WEIGHT_2X: f32 = 0.6;
const S: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Ce que la console attend d'une manette qui bouge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadMotion {
    /// rad/s.
    pub gyro: [f32; 3],
    /// En g.
    pub accel: [f32; 3],
    /// Quaternion x, y, z, w.
    pub orient: [f32; 4],
}

/// Suivi de l'orientation (`ChiakiOrientationTracker`).
#[derive(Debug, Clone)]
pub struct MotionTracker {
    gyro: [f32; 3],
    accel: [f32; 3],
    /// w, x, y, z (repère de Madgwick, tourné de 90° autour de x).
    q: [f32; 4],
    timestamp_us: u64,
    samples: u64,
}

impl Default for MotionTracker {
    fn default() -> Self {
        MotionTracker { gyro: [0.0; 3], accel: [0.0, 1.0, 0.0], q: [S, S, 0.0, 0.0], timestamp_us: 0, samples: 0 }
    }
}

fn fuzz(cur: f32, prev: &mut f32) {
    let f = ORIENT_FUZZ;
    if cur < *prev + f / 2.0 && cur > *prev - f / 2.0 {
        return;
    }
    if cur < *prev + f && cur > *prev - f {
        *prev = FUZZ_PREV_WEIGHT * *prev + (1.0 - FUZZ_PREV_WEIGHT) * cur;
        return;
    }
    if cur < *prev + 2.0 * f && cur > *prev - 2.0 * f {
        *prev = FUZZ_PREV_WEIGHT_2X * *prev + (1.0 - FUZZ_PREV_WEIGHT_2X) * cur;
        return;
    }
    *prev = cur;
}

impl MotionTracker {
    /// Une mesure du gyroscope (rad/s) : les dernières valeurs de
    /// l'accéléromètre s'y joignent.
    pub fn gyro(&mut self, g: [f32; 3], timestamp_us: u64) {
        self.update(g, self.accel, timestamp_us);
    }

    /// Une mesure de l'accéléromètre (m/s²).
    pub fn accel(&mut self, a: [f32; 3], timestamp_us: u64) {
        self.update(self.gyro, a.map(|v| v / GRAVITY), timestamp_us);
    }

    fn update(&mut self, g: [f32; 3], a: [f32; 3], timestamp_us: u64) {
        self.gyro = g;
        self.accel = a;
        self.samples += 1;
        if self.samples <= 1 {
            self.timestamp_us = timestamp_us;
            return;
        }
        let dt = timestamp_us.saturating_sub(self.timestamp_us) as f32 / 1_000_000.0;
        self.timestamp_us = timestamp_us;
        let beta = if self.samples < WARMUP_SAMPLES { BETA_WARMUP } else { BETA_DEFAULT };
        self.madgwick(g, a, beta, dt);
    }

    /// `chiaki_orientation_update`.
    fn madgwick(&mut self, [gx, gy, gz]: [f32; 3], [mut ax, mut ay, mut az]: [f32; 3], beta: f32, dt: f32) {
        let [mut q0, mut q1, mut q2, mut q3] = self.q;
        let mut d0 = 0.5 * (-q1 * gx - q2 * gy - q3 * gz);
        let mut d1 = 0.5 * (q0 * gx + q2 * gz - q3 * gy);
        let mut d2 = 0.5 * (q0 * gy - q1 * gz + q3 * gx);
        let mut d3 = 0.5 * (q0 * gz + q1 * gy - q2 * gx);
        if !(ax == 0.0 && ay == 0.0 && az == 0.0) {
            let n = 1.0 / (ax * ax + ay * ay + az * az).sqrt();
            ax *= n;
            ay *= n;
            az *= n;
            let (q0q0, q1q1, q2q2, q3q3) = (q0 * q0, q1 * q1, q2 * q2, q3 * q3);
            let mut s0 = 4.0 * q0 * q2q2 + 2.0 * q2 * ax + 4.0 * q0 * q1q1 - 2.0 * q1 * ay;
            let mut s1 = 4.0 * q1 * q3q3 - 2.0 * q3 * ax + 4.0 * q0q0 * q1 - 2.0 * q0 * ay - 4.0 * q1 + 8.0 * q1 * q1q1 + 8.0 * q1 * q2q2 + 4.0 * q1 * az;
            let mut s2 = 4.0 * q0q0 * q2 + 2.0 * q0 * ax + 4.0 * q2 * q3q3 - 2.0 * q3 * ay - 4.0 * q2 + 8.0 * q2 * q1q1 + 8.0 * q2 * q2q2 + 4.0 * q2 * az;
            let mut s3 = 4.0 * q1q1 * q3 - 2.0 * q1 * ax + 4.0 * q2q2 * q3 - 2.0 * q2 * ay;
            let norm = s0 * s0 + s1 * s1 + s2 * s2 + s3 * s3;
            if norm > 0.000001 {
                let r = 1.0 / norm.sqrt();
                s0 *= r;
                s1 *= r;
                s2 *= r;
                s3 *= r;
                d0 -= beta * s0;
                d1 -= beta * s1;
                d2 -= beta * s2;
                d3 -= beta * s3;
            }
        }
        q0 += d0 * dt;
        q1 += d1 * dt;
        q2 += d2 * dt;
        q3 += d3 * dt;
        let r = 1.0 / (q0 * q0 + q1 * q1 + q2 * q2 + q3 * q3).sqrt();
        let next = [q0 * r, q1 * r, q2 * r, q3 * r];
        for (i, v) in next.into_iter().enumerate() {
            fuzz(v, &mut self.q[i]);
        }
    }

    /// Mesures et orientation pour la console (repère tourné de -90° autour
    /// de x, `chiaki_orientation_tracker_apply_to_controller_state`).
    pub fn motion(&self) -> PadMotion {
        let [w, x, y, z] = self.q;
        let (c, s) = (S, -S);
        PadMotion {
            gyro: self.gyro,
            accel: self.accel,
            orient: [c * x + s * w, c * y - s * z, c * z + s * y, c * w - s * x],
        }
    }

    /// Une mesure a déjà été reçue.
    pub fn active(&self) -> bool {
        self.samples > 0
    }
}

/// Le pavé tactile : clic et deux doigts au plus (positions de 0 à 1).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadTouch {
    pub clicked: bool,
    pub fingers: [Option<(f32, f32)>; 2],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_and_still_stays_level() {
        let mut t = MotionTracker::default();
        for i in 0..400u64 {
            t.accel([0.0, GRAVITY, 0.0], i * 4000);
            t.gyro([0.0; 3], i * 4000 + 2000);
        }
        let m = t.motion();
        // Manette à plat : orientation de repos (quaternion identité).
        assert!((m.orient[3] - 1.0).abs() < 0.01, "{:?}", m.orient);
        assert!(m.orient[0].abs() < 0.01 && m.orient[1].abs() < 0.01 && m.orient[2].abs() < 0.01);
        assert!((m.accel[1] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn rotation_moves_orientation() {
        let mut t = MotionTracker::default();
        for i in 0..100u64 {
            t.accel([0.0, GRAVITY, 0.0], i * 4000);
        }
        let before = t.motion().orient;
        // Une seconde à 1 rad/s autour de l'axe vertical, après l'échauffement.
        for i in 0..250u64 {
            t.gyro([0.0, 1.0, 0.0], 400_000 + i * 4000);
        }
        assert_ne!(t.motion().orient, before);
        assert_eq!(t.motion().gyro, [0.0, 1.0, 0.0]);
    }
}
