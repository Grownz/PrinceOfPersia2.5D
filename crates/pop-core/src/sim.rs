//! Deterministic game simulation. The original game runs its logic at 12 ticks per second.
//! We step at 60 Hz (`SUBSTEPS` = 5 steps per logic tick): the original logic runs unchanged on every
//! 5th step, the steps in between only advance `alpha()` so rendering can interpolate and input can
//! be sampled at 60 Hz. Mechanics therefore stay identical to the original.
pub const TICKS_PER_SEC: u32 = 12;
pub const SUBSTEPS: u32 = 5;
pub const STEP_HZ: f64 = (TICKS_PER_SEC * SUBSTEPS) as f64;

#[derive(Default)]
pub struct World {
    /// original logic ticks (12 Hz)
    pub tick: u64,
    sub: u32,
}

impl World {
    /// One 60 Hz step; returns true if a logic tick ran.
    pub fn step(&mut self) -> bool {
        self.sub += 1;
        if self.sub < SUBSTEPS {
            return false;
        }
        self.sub = 0;
        self.tick += 1;
        // TODO: port kid/guard state machines, tile triggers (gates, plates, loose floors, spikes).
        true
    }

    /// Progress from the last logic tick to the next, in [0, 1), for render interpolation.
    pub fn alpha(&self) -> f32 {
        self.sub as f32 / SUBSTEPS as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logic_runs_every_fifth_step() {
        let mut w = World::default();
        let ran: Vec<bool> = (0..10).map(|_| w.step()).collect();
        assert_eq!(ran, [false, false, false, false, true, false, false, false, false, true]);
        assert_eq!(w.tick, 2);
        assert_eq!(w.alpha(), 0.0);
        w.step();
        assert_eq!(w.alpha(), 0.2);
        assert_eq!(STEP_HZ, 60.0);
    }
}
