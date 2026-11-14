//! Deterministic game simulation. The original runs its logic at 12 ticks per second;
//! rendering interpolates between ticks, so mechanics stay original while visuals are smooth.
pub const TICKS_PER_SEC: f64 = 12.0;

#[derive(Default)]
pub struct World {
    pub tick: u64,
}

impl World {
    pub fn step(&mut self) {
        self.tick += 1;
        // TODO: port kid/guard state machines, tile triggers (gates, plates, loose floors, spikes).
    }
}
