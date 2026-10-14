use crate::ProtocolError;

pub(crate) fn read_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

pub(crate) fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

pub(crate) fn read_f32(bytes: &[u8]) -> f32 {
    f32::from_bits(read_u32(bytes))
}

pub(crate) fn read_f32_array<const N: usize>(bytes: &[u8]) -> Result<[f32; N], ProtocolError> {
    let mut values = [0.0; N];
    for (index, value) in values.iter_mut().enumerate() {
        *value = finite(read_f32(&bytes[index * 4..index * 4 + 4]))?;
    }
    Ok(values)
}

pub(crate) fn read_u32_array<const N: usize>(bytes: &[u8]) -> [u32; N] {
    let mut values = [0; N];
    for (index, value) in values.iter_mut().enumerate() {
        *value = read_u32(&bytes[index * 4..index * 4 + 4]);
    }
    values
}

pub(crate) fn finite(value: f32) -> Result<f32, ProtocolError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(ProtocolError::InvalidNumber)
}

pub(crate) fn nonnegative(value: f32) -> Result<f32, ProtocolError> {
    let value = finite(value)?;
    (value >= 0.0)
        .then_some(value)
        .ok_or(ProtocolError::InvalidNumber)
}

pub(crate) fn nonnegative_array<const N: usize>(
    values: [f32; N],
) -> Result<[f32; N], ProtocolError> {
    if values.iter().all(|value| *value >= 0.0) {
        Ok(values)
    } else {
        Err(ProtocolError::InvalidNumber)
    }
}
