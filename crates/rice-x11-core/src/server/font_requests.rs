//! Font requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn open_font(&mut self, handle: ClientHandle, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::open_font_request(body).ok_or_else(bad_length)?;
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(request.font) || self.resource_exists(request.font) {
            return Err(ProtocolError::new(
                x11::error::BAD_ID_CHOICE,
                request.font.0,
            ));
        }
        if request.name.is_empty() {
            return Err(ProtocolError::new(x11::error::BAD_NAME, 0));
        }
        self.fonts.insert(
            request.font.0,
            Font {
                owner: handle.0,
                name: request.name,
            },
        );
        Ok(Vec::new())
    }

    pub(super) fn close_font(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let font = x11::free_resource_id(body).ok_or_else(bad_length)?;
        if self.fonts.remove(&font.0).is_none() {
            return Err(ProtocolError::new(x11::error::BAD_FONT, font.0));
        }
        for gc in self.graphics_contexts.values_mut() {
            if gc.font == Some(font) {
                gc.font = None;
            }
        }
        Ok(Vec::new())
    }

    pub(super) fn query_font(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let fontable = request_resource(body).ok_or_else(bad_length)?;
        if self
            .fonts
            .get(&fontable.0)
            .is_none_or(|font| font.name.is_empty())
            && !self.graphics_contexts.contains_key(&fontable.0)
        {
            return Err(ProtocolError::new(x11::error::BAD_FONT, fontable.0));
        }
        let mut output = Vec::new();
        x11::write_query_font_reply(&mut output, order, sequence, &builtin_font_metrics())
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn query_text_extents(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        odd_length: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::query_text_extents_request(odd_length, body).ok_or_else(bad_length)?;
        if self
            .fonts
            .get(&request.fontable.0)
            .is_none_or(|font| font.name.is_empty())
            && !self.graphics_contexts.contains_key(&request.fontable.0)
        {
            return Err(ProtocolError::new(x11::error::BAD_FONT, request.fontable.0));
        }
        let width = i32::try_from(request.chars.len())
            .unwrap_or(i32::MAX)
            .saturating_mul(8);
        let mut output = Vec::with_capacity(32);
        x11::write_query_text_extents_reply(
            &mut output,
            order,
            sequence,
            x11::TextExtents {
                draw_direction: 0,
                font_ascent: 9,
                font_descent: 3,
                overall_ascent: 9,
                overall_descent: 3,
                overall_width: width,
                overall_left: 0,
                overall_right: width,
            },
        )
        .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn list_fonts(
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::list_fonts_request(body).ok_or_else(bad_length)?;
        let names = matching_builtin_fonts(&request.pattern, request.max_names);
        Ok(encode_list_fonts_reply(order, sequence, &names))
    }

    pub(super) fn list_fonts_with_info(
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::list_fonts_request(body).ok_or_else(bad_length)?;
        let names = matching_builtin_fonts(&request.pattern, request.max_names);
        let metrics = builtin_font_metrics();
        let mut output = Vec::new();
        for (index, name) in names.iter().enumerate() {
            x11::write_list_fonts_with_info_reply(
                &mut output,
                order,
                sequence,
                &metrics,
                name,
                u32::try_from(names.len().saturating_sub(index + 1)).unwrap_or(0),
            )
            .map_err(|_| encoding_error())?;
        }
        x11::write_list_fonts_with_info_terminator(&mut output, order, sequence)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn get_font_path(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
        let mut output = Vec::with_capacity(32);
        output.push(1);
        output.push(0);
        let mut sequence_bytes = [0; 2];
        write_u16_at(order, &mut sequence_bytes, sequence.0);
        output.extend_from_slice(&sequence_bytes);
        write_u32_vec(order, &mut output, 0);
        output.extend_from_slice(&[0; 24]);
        output
    }
}
