use anyhow::{Context, Result, bail};
use can_dbc::{ByteOrder, Dbc, MessageId, ValueType};
use lumen_common::{EngineRpm, WheelSpeeds};

pub const EMS_DCT11_ID: u32 = 128;
pub const WHL_SPD11_ID: u32 = 902;

pub fn encode_engine(dbc: &Dbc, rpm: EngineRpm) -> Result<(u32, [u8; 8])> {
    let mut frame = [0u8; 8];
    write_signal(dbc, &mut frame, EMS_DCT11_ID, "N", rpm.0 as f64)?;
    Ok((EMS_DCT11_ID, frame))
}

pub fn encode_wheel_speeds(dbc: &Dbc, w: WheelSpeeds) -> Result<(u32, [u8; 8])> {
    let mut frame = [0u8; 8];
    for (name, value) in [
        ("WHL_SPD_FL", w.front_left),
        ("WHL_SPD_FR", w.front_right),
        ("WHL_SPD_RL", w.rear_left),
        ("WHL_SPD_RR", w.rear_right),
    ] {
        write_signal(dbc, &mut frame, WHL_SPD11_ID, name, value as f64)?;
    }
    Ok((WHL_SPD11_ID, frame))
}

fn write_signal(
    dbc: &Dbc,
    frame: &mut [u8; 8],
    message_id: u32,
    signal_name: &str,
    physical: f64,
) -> Result<()> {
    let signal = dbc
        .signal_by_name(MessageId::Standard(message_id as u16), signal_name)
        .with_context(|| format!("signal {signal_name} not found in message 0x{message_id:X}"))?;

    let raw = ((physical - signal.offset) / signal.factor).round() as i64;
    let raw_bits = clamp_to_size(raw, signal.size, &signal.value_type)?;

    match signal.byte_order {
        ByteOrder::LittleEndian => pack_le(frame, signal.start_bit, signal.size, raw_bits),
        ByteOrder::BigEndian => bail!("big-endian signals not yet supported ({signal_name})"),
    }

    Ok(())
}

fn clamp_to_size(raw: i64, size: u64, value_type: &ValueType) -> Result<u64> {
    match value_type {
        ValueType::Unsigned => {
            if raw < 0 {
                bail!("unsigned signal received negative raw value {raw}");
            }
            let max: u64 = if size >= 64 { u64::MAX } else { (1u64 << size) - 1 };
            Ok((raw as u64).min(max))
        }
        ValueType::Signed => {
            let half = 1i64 << (size - 1);
            let clamped = raw.clamp(-half, half - 1);
            let mask: u64 = if size >= 64 { u64::MAX } else { (1u64 << size) - 1 };
            Ok((clamped as u64) & mask)
        }
    }
}

/// Pack `size` low bits of `raw` into `frame`, starting at `start_bit`.
/// Little-endian (Intel) bit order: successive signal bits occupy successive
/// payload bit indices, where bit N of the payload = bit (N % 8) of byte (N / 8).
fn pack_le(frame: &mut [u8], start_bit: u64, size: u64, raw: u64) {
    for i in 0..size {
        let bit_index = start_bit + i;
        let byte_index = (bit_index / 8) as usize;
        let bit_in_byte = (bit_index % 8) as u8;
        let val = ((raw >> i) & 1) as u8;
        frame[byte_index] |= val << bit_in_byte;
    }
}
