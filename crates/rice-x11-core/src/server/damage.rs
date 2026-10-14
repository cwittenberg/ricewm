//! Damage operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn damage_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = match minor {
            x11damage::QUERY_VERSION => {
                if body.len() == 8 {
                    Ok(x11damage::encode_query_version_reply(
                        order,
                        sequence,
                        x11damage::MAJOR_VERSION,
                        x11damage::MINOR_VERSION,
                    ))
                } else {
                    Err(bad_length())
                }
            }
            x11damage::CREATE => {
                if body.len() != 12 {
                    return Err(extension_error(bad_length(), minor));
                }
                let id = ResourceId(read_u32(order, &body[0..4]));
                let drawable = ResourceId(read_u32(order, &body[4..8]));
                let level = body[8];
                let client = self
                    .clients
                    .get(&handle.0)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
                if !client.owns_resource(id) || self.resource_exists(id) {
                    Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0))
                } else if !self.surfaces.contains_key(&drawable.0) {
                    Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))
                } else if level > x11damage::report_level::NON_EMPTY {
                    Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(level)))
                } else {
                    self.damage_resources.insert(
                        id.0,
                        DamageResource {
                            owner: handle.0,
                            drawable,
                            level,
                            pending: Vec::new(),
                        },
                    );
                    Ok(Vec::new())
                }
            }
            x11damage::DESTROY => {
                let id = decode_u32_request(order, body)?;
                if self.damage_resources.remove(&id).is_some() {
                    Ok(Vec::new())
                } else {
                    Err(ProtocolError::new(DAMAGE_FIRST_ERROR, id))
                }
            }
            x11damage::SUBTRACT => self.damage_subtract(order, body),
            x11damage::ADD => self.damage_add(order, body),
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        };
        result.map_err(|error| extension_error(error, minor))
    }

    pub(super) fn damage_subtract(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 12 {
            return Err(bad_length());
        }
        let id = read_u32(order, &body[0..4]);
        let repair = read_u32(order, &body[4..8]);
        let parts = read_u32(order, &body[8..12]);
        if repair != 0 && !self.xfixes_regions.contains_key(&repair) {
            return Err(ProtocolError::new(XFIXES_FIRST_ERROR, repair));
        }
        if parts != 0 && !self.xfixes_regions.contains_key(&parts) {
            return Err(ProtocolError::new(XFIXES_FIRST_ERROR, parts));
        }
        let pending = self
            .damage_resources
            .get(&id)
            .map(|damage| damage.pending.clone())
            .ok_or_else(|| ProtocolError::new(DAMAGE_FIRST_ERROR, id))?;
        if parts != 0
            && let Some(region) = self.xfixes_regions.get_mut(&parts)
        {
            region.rectangles = normalize_region(pending.clone());
        }
        let remaining = if repair == 0 {
            Vec::new()
        } else {
            let repairs = self
                .xfixes_regions
                .get(&repair)
                .map(|region| region.rectangles.clone())
                .unwrap_or_default();
            subtract_region(&pending, &repairs)
        };
        if let Some(damage) = self.damage_resources.get_mut(&id) {
            damage.pending = remaining;
        }
        Ok(Vec::new())
    }

    pub(super) fn damage_add(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let drawable = ResourceId(read_u32(order, &body[0..4]));
        let region = read_u32(order, &body[4..8]);
        if !self.surfaces.contains_key(&drawable.0) {
            return Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0));
        }
        let rectangles = self
            .xfixes_regions
            .get(&region)
            .map(|region| region.rectangles.clone())
            .ok_or_else(|| ProtocolError::new(XFIXES_FIRST_ERROR, region))?;
        self.record_damage(drawable, &rectangles);
        Ok(Vec::new())
    }
}
