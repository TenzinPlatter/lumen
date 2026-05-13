use lumen_common::{EngineRpm, WheelSpeeds};

pub struct SimState {
    pub rpm: f32,
    pub speed_kmh: f32,
    /// Direction in [-1, +1] from the currently-held arrow keys. Each tick the
    /// state values advance by `rate * direction * dt`, giving pedal-style
    /// continuous acceleration while held.
    pedal_rpm: f32,
    pedal_speed: f32,
}

impl SimState {
    /// Rate of change while a pedal is fully held.
    /// 2500 rpm/s ≈ idle → redline in ~3 s. 15 km/h/s ≈ 0–100 km/h in ~7 s.
    const RPM_PEDAL_RATE: f32 = 2500.0;
    const SPEED_PEDAL_RATE: f32 = 15.0;

    /// Fallback per-keypress nudge for terminals without the kitty keyboard
    /// protocol — relies on the OS auto-repeat for "hold" behavior.
    pub const RPM_NUDGE: f32 = 50.0;
    pub const SPEED_NUDGE: f32 = 0.5;

    const RPM_MAX: f32 = 7000.0;
    const SPEED_MAX_KMH: f32 = 200.0;
    const RPM_IDLE: f32 = 800.0;

    pub fn new() -> Self {
        Self {
            rpm: Self::RPM_IDLE,
            speed_kmh: 0.0,
            pedal_rpm: 0.0,
            pedal_speed: 0.0,
        }
    }

    pub fn tick(&mut self, dt_secs: f32) {
        if self.pedal_rpm != 0.0 {
            self.rpm = (self.rpm + Self::RPM_PEDAL_RATE * self.pedal_rpm * dt_secs)
                .clamp(0.0, Self::RPM_MAX);
        }
        if self.pedal_speed != 0.0 {
            self.speed_kmh = (self.speed_kmh + Self::SPEED_PEDAL_RATE * self.pedal_speed * dt_secs)
                .clamp(0.0, Self::SPEED_MAX_KMH);
        }
    }

    pub fn set_rpm_pedal(&mut self, direction: f32) {
        self.pedal_rpm = direction.clamp(-1.0, 1.0);
    }

    pub fn set_speed_pedal(&mut self, direction: f32) {
        self.pedal_speed = direction.clamp(-1.0, 1.0);
    }

    pub fn nudge_rpm(&mut self, delta: f32) {
        self.rpm = (self.rpm + delta).clamp(0.0, Self::RPM_MAX);
    }

    pub fn nudge_speed(&mut self, delta: f32) {
        self.speed_kmh = (self.speed_kmh + delta).clamp(0.0, Self::SPEED_MAX_KMH);
    }

    pub fn coast(&mut self) {
        self.pedal_rpm = 0.0;
        self.pedal_speed = 0.0;
        self.rpm = Self::RPM_IDLE;
        self.speed_kmh = 0.0;
    }

    pub fn engine(&self) -> EngineRpm {
        EngineRpm(self.rpm)
    }

    pub fn wheels(&self) -> WheelSpeeds {
        WheelSpeeds {
            front_left: self.speed_kmh,
            front_right: self.speed_kmh,
            rear_left: self.speed_kmh,
            rear_right: self.speed_kmh,
        }
    }
}
