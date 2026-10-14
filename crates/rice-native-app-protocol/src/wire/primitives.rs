use crate::{
    AppId, DragId, MAX_PATH_BYTES, MAX_TEXT_BYTES, PatchValue, RequestId, WidgetId, WindowId,
};

use super::{DecodeError, EncodeError};

#[derive(Default)]
pub(super) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn id<T: ProtocolId>(&mut self, id: T) {
        self.u64(id.get());
    }

    pub(super) fn option_id<T: ProtocolId>(&mut self, value: Option<T>) {
        self.u64(value.map_or(0, ProtocolId::get));
    }

    pub(super) fn option_u16(&mut self, value: Option<u16>) {
        self.u8(u8::from(value.is_some()));
        if let Some(value) = value {
            self.u16(value);
        }
    }

    pub(super) fn patch_u16(&mut self, value: &PatchValue<u16>) {
        match value {
            PatchValue::Keep => self.u8(0),
            PatchValue::Set(value) => {
                self.u8(1);
                self.u16(*value);
            }
            PatchValue::Clear => self.u8(2),
        }
    }

    pub(super) fn patch_u32(&mut self, value: &PatchValue<u32>) {
        match value {
            PatchValue::Keep => self.u8(0),
            PatchValue::Set(value) => {
                self.u8(1);
                self.u32(*value);
            }
            PatchValue::Clear => self.u8(2),
        }
    }

    pub(super) fn option_u64(&mut self, value: Option<u64>) {
        self.u8(u8::from(value.is_some()));
        if let Some(value) = value {
            self.u64(value);
        }
    }

    pub(super) fn count(
        &mut self,
        count: usize,
        maximum: usize,
        field: &'static str,
    ) -> Result<(), EncodeError> {
        if count > maximum {
            return Err(EncodeError::LimitExceeded {
                field,
                actual: count,
                maximum,
            });
        }
        self.u32(u32::try_from(count).map_err(|_| EncodeError::InvalidValue(field))?);
        Ok(())
    }

    pub(super) fn string(
        &mut self,
        value: &str,
        maximum: usize,
        field: &'static str,
    ) -> Result<(), EncodeError> {
        self.bytes(value.as_bytes(), maximum, field)
    }

    pub(super) fn path(&mut self, value: &str) -> Result<(), EncodeError> {
        self.string(value, MAX_PATH_BYTES, "path")
    }

    pub(super) fn option_string(
        &mut self,
        value: Option<&str>,
        maximum: usize,
        field: &'static str,
    ) -> Result<(), EncodeError> {
        self.u8(u8::from(value.is_some()));
        if let Some(value) = value {
            self.string(value, maximum, field)?;
        }
        Ok(())
    }

    pub(super) fn patch_string(
        &mut self,
        value: &PatchValue<String>,
        maximum: usize,
        field: &'static str,
    ) -> Result<(), EncodeError> {
        match value {
            PatchValue::Keep => self.u8(0),
            PatchValue::Set(value) => {
                self.u8(1);
                self.string(value, maximum, field)?;
            }
            PatchValue::Clear => self.u8(2),
        }
        Ok(())
    }

    pub(super) fn string_list(
        &mut self,
        values: &[String],
        maximum: usize,
        field: &'static str,
    ) -> Result<(), EncodeError> {
        self.count(values.len(), maximum, field)?;
        for value in values {
            self.string(value, MAX_TEXT_BYTES, field)?;
        }
        Ok(())
    }

    pub(super) fn bytes(
        &mut self,
        value: &[u8],
        maximum: usize,
        field: &'static str,
    ) -> Result<(), EncodeError> {
        if value.len() > maximum {
            return Err(EncodeError::LimitExceeded {
                field,
                actual: value.len(),
                maximum,
            });
        }
        self.u32(u32::try_from(value.len()).map_err(|_| EncodeError::InvalidValue(field))?);
        self.bytes.extend_from_slice(value);
        Ok(())
    }
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    pub(super) fn take(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(DecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(DecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn bool(&mut self, field: &'static str) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::InvalidValue(field)),
        }
    }

    pub(super) fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub(super) fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub(super) fn i64(&mut self) -> Result<i64, DecodeError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub(super) fn id<T: ProtocolId>(&mut self, field: &'static str) -> Result<T, DecodeError> {
        T::new(self.u64()?).ok_or(DecodeError::InvalidValue(field))
    }

    pub(super) fn option_id<T: ProtocolId>(
        &mut self,
        field: &'static str,
    ) -> Result<Option<T>, DecodeError> {
        let value = self.u64()?;
        if value == 0 {
            Ok(None)
        } else {
            T::new(value)
                .map(Some)
                .ok_or(DecodeError::InvalidValue(field))
        }
    }

    pub(super) fn option_u16(&mut self) -> Result<Option<u16>, DecodeError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u16()?)),
            _ => Err(DecodeError::InvalidValue("optional u16 tag")),
        }
    }

    pub(super) fn patch_u16(&mut self) -> Result<PatchValue<u16>, DecodeError> {
        match self.u8()? {
            0 => Ok(PatchValue::Keep),
            1 => Ok(PatchValue::Set(self.u16()?)),
            2 => Ok(PatchValue::Clear),
            _ => Err(DecodeError::InvalidValue("patch u16 tag")),
        }
    }

    pub(super) fn patch_u32(&mut self) -> Result<PatchValue<u32>, DecodeError> {
        match self.u8()? {
            0 => Ok(PatchValue::Keep),
            1 => Ok(PatchValue::Set(self.u32()?)),
            2 => Ok(PatchValue::Clear),
            _ => Err(DecodeError::InvalidValue("patch u32 tag")),
        }
    }

    pub(super) fn option_u64(&mut self) -> Result<Option<u64>, DecodeError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u64()?)),
            _ => Err(DecodeError::InvalidValue("optional u64 tag")),
        }
    }

    pub(super) fn count(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<usize, DecodeError> {
        let count = usize::try_from(self.u32()?).map_err(|_| DecodeError::InvalidValue(field))?;
        if count > maximum {
            return Err(DecodeError::LimitExceeded {
                field,
                actual: count,
                maximum,
            });
        }
        Ok(count)
    }

    pub(super) fn string(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<String, DecodeError> {
        let bytes = self.bytes(maximum, field)?;
        let value = std::str::from_utf8(&bytes).map_err(|_| DecodeError::InvalidUtf8(field))?;
        Ok(value.to_owned())
    }

    pub(super) fn path(&mut self) -> Result<String, DecodeError> {
        self.string(MAX_PATH_BYTES, "path")
    }

    pub(super) fn option_string(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<Option<String>, DecodeError> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.string(maximum, field).map(Some),
            _ => Err(DecodeError::InvalidValue("optional string tag")),
        }
    }

    pub(super) fn patch_string(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<PatchValue<String>, DecodeError> {
        match self.u8()? {
            0 => Ok(PatchValue::Keep),
            1 => self.string(maximum, field).map(PatchValue::Set),
            2 => Ok(PatchValue::Clear),
            _ => Err(DecodeError::InvalidValue("patch string tag")),
        }
    }

    pub(super) fn string_list(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<Vec<String>, DecodeError> {
        let count = self.count(maximum, field)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.string(MAX_TEXT_BYTES, field)?);
        }
        Ok(values)
    }

    pub(super) fn bytes(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<Vec<u8>, DecodeError> {
        let length = usize::try_from(self.u32()?).map_err(|_| DecodeError::InvalidValue(field))?;
        if length > maximum {
            return Err(DecodeError::LimitExceeded {
                field,
                actual: length,
                maximum,
            });
        }
        Ok(self.take(length)?.to_vec())
    }
}

pub(super) trait ProtocolId: Copy {
    fn new(value: u64) -> Option<Self>;
    fn get(self) -> u64;
}

macro_rules! protocol_id {
    ($($name:ty),+ $(,)?) => {
        $(impl ProtocolId for $name {
            fn new(value: u64) -> Option<Self> { Self::new(value) }
            fn get(self) -> u64 { self.get() }
        })+
    };
}

protocol_id!(AppId, WindowId, WidgetId, RequestId, DragId);
