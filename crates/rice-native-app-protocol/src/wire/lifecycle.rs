use super::{
    DecodeError,
    primitives::{Reader, Writer},
};
use crate::{CloseDecision, CloseReason, CloseRequest, CloseResponse};

pub(super) fn encode_request(writer: &mut Writer, request: &CloseRequest) {
    writer.id(request.request);
    writer.id(request.app);
    writer.u8(request.reason as u8);
}

pub(super) fn decode_request(reader: &mut Reader<'_>) -> Result<CloseRequest, DecodeError> {
    Ok(CloseRequest {
        request: reader.id("close request id")?,
        app: reader.id("app id")?,
        reason: decode_reason(reader)?,
    })
}

pub(super) fn encode_response(writer: &mut Writer, response: &CloseResponse) {
    writer.id(response.request);
    writer.id(response.app);
    writer.u8(response.decision as u8);
}

pub(super) fn decode_response(reader: &mut Reader<'_>) -> Result<CloseResponse, DecodeError> {
    let request = reader.id("close request id")?;
    let app = reader.id("app id")?;
    let decision = match reader.u8()? {
        0 => CloseDecision::Approved,
        1 => CloseDecision::Pending,
        2 => CloseDecision::Cancelled,
        value => {
            return Err(DecodeError::UnknownTag {
                field: "close decision",
                value,
            });
        }
    };
    Ok(CloseResponse {
        request,
        app,
        decision,
    })
}

pub(super) fn decode_reason(reader: &mut Reader<'_>) -> Result<CloseReason, DecodeError> {
    match reader.u8()? {
        0 => Ok(CloseReason::Requested),
        1 => Ok(CloseReason::LastWindowClosed),
        2 => Ok(CloseReason::HostShutdown),
        3 => Ok(CloseReason::ProtocolFault),
        value => Err(DecodeError::UnknownTag {
            field: "close reason",
            value,
        }),
    }
}
