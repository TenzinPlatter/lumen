use serde::{Deserialize, Serialize};

/// Default Unix domain socket path the simulator binds and the HUD connects to.
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/lumen.sock";

/// A single state snapshot crossing the process boundary from the producer
/// (simulator or hardware host) to the HUD. Sent as newline-delimited JSON
/// over the UDS at [`DEFAULT_SOCKET_PATH`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HudSnapshot {
    pub rpm: f32,
    pub speed_kmh: f32,
}

/// Engine speed in revolutions per minute.
///
/// Sourced from the engine ECU's `EMS_DCT11.N` signal (CAN ID 0x080),
/// which has a raw range of 0..=16383.8 rpm at 0.25 rpm resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineRpm(pub f32);

/// The four wheel speeds in km/h, as broadcast by the ABS module in
/// `WHL_SPD11` (CAN ID 0x386). Resolution is 0.03125 km/h per wheel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelSpeeds {
    pub front_left: f32,
    pub front_right: f32,
    pub rear_left: f32,
    pub rear_right: f32,
}

impl WheelSpeeds {
    /// Vehicle speed as the factory cluster derives it: the average of the
    /// non-driven wheels. Assumes FWD
    pub fn cluster_speed(&self) -> f32 {
        (self.rear_left + self.rear_right) / 2.0
    }
}
