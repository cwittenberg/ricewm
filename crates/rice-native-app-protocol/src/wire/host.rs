use crate::{
    HostEvent, HostEventKind, HostOperation, HostRequest, ImageExportAction, MediaCommand,
    TerminalAction, TerminalEvent,
};

use super::{DecodeError, EncodeError, Reader, Writer};

const MAX_RICE_BUS_FILTERS: usize = 8;
const MIN_HOST_TIMER_INTERVAL_MILLIS: u32 = 16;

pub(super) fn encode_request(
    writer: &mut Writer,
    request: &HostRequest,
) -> Result<(), EncodeError> {
    writer.id(request.app);
    writer.id(request.window);
    match &request.operation {
        HostOperation::SetContentSize { token, width, height, anchor } => {
            if *anchor > 3 || *token == 0 || !(96..=4096).contains(width) || !(96..=4096).contains(height) { return Err(EncodeError::InvalidValue("window content size")); }
            writer.u8(43); writer.u64(*token); writer.u32(*width); writer.u32(*height); writer.u8(*anchor);
        }
        HostOperation::DataRequestFence { token, resume } => {
            if *token==0 {return Err(EncodeError::InvalidValue("data request fence"));}
            writer.u8(42);writer.u64(*token);writer.u8(u8::from(*resume));
        }
        HostOperation::InteractiveNotification { payload } => {
            writer.u8(41);
            writer.string(payload, 256 * 1024, "interactive notification")?;
        }
        HostOperation::PaddyWhatsApp { payload } => {
            writer.u8(40);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "Paddy WhatsApp command")?;
        }
        HostOperation::SetFullscreen(enabled) => {
            writer.u8(0);
            writer.u8(u8::from(*enabled));
        }
        HostOperation::SetTimer {
            token,
            interval_millis,
            enabled,
        } => {
            if *token == 0
                || *interval_millis < MIN_HOST_TIMER_INTERVAL_MILLIS
                || *interval_millis > 86_400_000
            {
                return Err(EncodeError::InvalidValue("host timer"));
            }
            writer.u8(1);
            writer.u64(*token);
            writer.u32(*interval_millis);
            writer.u8(u8::from(*enabled));
        }
        HostOperation::RiceBusSubscribe { topic, filters } => {
            if topic.is_empty() {
                return Err(EncodeError::InvalidValue("RiceBus topic"));
            }
            writer.u8(30);
            writer.string(topic, crate::MAX_TEXT_BYTES, "RiceBus topic")?;
            writer.count(
                filters.len(),
                MAX_RICE_BUS_FILTERS,
                "RiceBus subscription filters",
            )?;
            for filter in filters {
                writer.string(filter, crate::MAX_PATH_BYTES, "RiceBus subscription filter")?;
            }
        }
        HostOperation::PrintText { title, text } => {
            writer.u8(2);
            writer.string(title, crate::MAX_TEXT_BYTES, "print title")?;
            writer.string(text, crate::MAX_TRANSFER_BYTES, "print text")?;
        }
        HostOperation::RequestClose => writer.u8(3),
        HostOperation::FileDialogResult { path } => {
            writer.u8(31);
            writer.u8(u8::from(path.is_some()));
            if let Some(path) = path { writer.string(path, crate::MAX_PATH_BYTES, "dialog path")?; }
        }
        HostOperation::MenuResult { id } => {
            writer.u8(37);
            writer.u8(u8::from(id.is_some()));
            if let Some(id) = id { writer.string(id, 256, "menu item ID")?; }
        }
        HostOperation::MediaControl {
            session,
            command,
            value,
        } => {
            if *session == 0 {
                return Err(EncodeError::InvalidValue("media session"));
            }
            writer.u8(4);
            writer.u64(*session);
            writer.u8(*command as u8);
            writer.u64(*value as u64);
        }
        HostOperation::MediaOpen {
            replace_session,
            path,
        } => {
            validate_media_path(path).map_err(EncodeError::InvalidValue)?;
            writer.u8(18);
            writer.u64(*replace_session);
            writer.string(path, crate::MAX_PATH_BYTES, "media path")?;
        }
        HostOperation::MediaOpenMuted { replace_session, path } => {
            validate_media_path(path).map_err(EncodeError::InvalidValue)?;
            writer.u8(38);
            writer.u64(*replace_session);
            writer.string(path, crate::MAX_PATH_BYTES, "media path")?;
        }
        HostOperation::ProtectedMedia {
            request,
            handle,
            action,
            metadata,
            data,
        } => {
            if *request == 0 || action.is_empty() {
                return Err(EncodeError::InvalidValue("protected media request"));
            }
            writer.u8(32);
            writer.u64(*request);
            writer.u64(*handle);
            writer.string(action, crate::MAX_TEXT_BYTES, "protected media action")?;
            writer.string(
                metadata,
                crate::MAX_HTTP_METADATA_BYTES,
                "protected media metadata",
            )?;
            writer.bytes(data, crate::MAX_HTTP_BODY_BYTES, "protected media data")?;
        }
        HostOperation::WebSocket {
            tab,
            realm,
            handle,
            action,
            metadata,
            data,
        } => {
            if *tab == 0 || *realm == 0 || *handle == 0 || !(1..=3).contains(action) ||
                data.len() > 1024 * 1024
            {
                return Err(EncodeError::InvalidValue("WebSocket request"));
            }
            writer.u8(35);
            writer.u64(*tab);
            writer.u64(*realm);
            writer.u64(*handle);
            writer.u8(*action);
            writer.string(metadata, 32 * 1024, "WebSocket metadata")?;
            writer.bytes(data, 1024 * 1024, "WebSocket data")?;
        }
        HostOperation::Geolocation {
            tab,
            request,
            watch,
            options,
        } => {
            if *tab == 0 || *request == 0 {
                return Err(EncodeError::InvalidValue("geolocation request"));
            }
            writer.u8(33);
            writer.u64(*tab);
            writer.u64(*request);
            writer.bool(*watch);
            writer.string(options, 1024, "geolocation options")?;
        }
        HostOperation::ClearGeolocation { tab, request } => {
            if *tab == 0 || *request == 0 {
                return Err(EncodeError::InvalidValue("geolocation watch"));
            }
            writer.u8(34);
            writer.u64(*tab);
            writer.u64(*request);
        }
        HostOperation::JavaScriptRealm {
            request,
            tab,
            realm,
            action,
            url,
            source,
        } => {
            if *request == 0 || *tab == 0 || *realm == 0 || action.is_empty() {
                return Err(EncodeError::InvalidValue("JavaScript realm request"));
            }
            writer.u8(36);
            writer.u64(*request);
            writer.u64(*tab);
            writer.u64(*realm);
            writer.string(action, 16, "JavaScript realm action")?;
            writer.string(url, crate::MAX_TEXT_BYTES, "JavaScript realm URL")?;
            writer.bytes(source, 32 * 1024 * 1024, "JavaScript realm source")?;
        }
        HostOperation::ImageExport {
            session,
            action,
            width,
            height,
            quarter_turns,
            preserve_original,
            crop,
            filename,
        } => {
            if *session == 0
                || *width == 0
                || *height == 0
                || *width > crate::MAX_MEDIA_DIMENSION
                || *height > crate::MAX_MEDIA_DIMENSION
                || *quarter_turns > 3
            {
                return Err(EncodeError::InvalidValue("image export"));
            }
            if let Some([x, y, w, h]) = crop {
                if *w == 0 || *h == 0 || u32::from(*x) + u32::from(*w) > 4096
                    || u32::from(*y) + u32::from(*h) > 4096 || *preserve_original {
                    return Err(EncodeError::InvalidValue("image crop"));
                }
            }
            writer.u8(if crop.is_some() { 39 } else { 16 });
            writer.u64(*session);
            writer.u8(*action as u8);
            writer.u16(*width);
            writer.u16(*height);
            writer.u8(*quarter_turns);
            writer.u8(u8::from(*preserve_original));
            writer.string(filename, crate::MAX_PATH_BYTES, "image export filename")?;
            if let Some(crop) = crop { for value in crop { writer.u16(*value); } }
        }
        HostOperation::PerformanceSnapshot => writer.u8(5),
        HostOperation::AccelerationDiagnostics => writer.u8(13),
        HostOperation::ProcessSignal { pid, signal } => {
            if *pid == 0 || *signal == 0 || *signal > 64 {
                return Err(EncodeError::InvalidValue("process signal"));
            }
            writer.u8(6);
            writer.u32(*pid);
            writer.u8(*signal);
        }
        HostOperation::ProcessNice { pid, nice } => {
            if *pid == 0 || !(-20..=19).contains(nice) {
                return Err(EncodeError::InvalidValue("process niceness"));
            }
            writer.u8(7);
            writer.u32(*pid);
            writer.u8(*nice as u8);
        }
        HostOperation::AppStore { payload } => {
            writer.u8(8);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "app store request")?;
        }
        HostOperation::DownloadText {
            filename,
            mime_type,
            text,
        } => {
            writer.u8(9);
            writer.string(filename, crate::MAX_TEXT_BYTES, "download filename")?;
            writer.string(mime_type, crate::MAX_TEXT_BYTES, "download MIME type")?;
            writer.string(text, crate::MAX_HTTP_BODY_BYTES, "download text")?;
        }
        HostOperation::DownloadFile {
            filename,
            mime_type,
            data,
        } => {
            writer.u8(19);
            writer.string(filename, crate::MAX_TEXT_BYTES, "download filename")?;
            writer.string(mime_type, crate::MAX_TEXT_BYTES, "download MIME type")?;
            writer.bytes(data, crate::MAX_HTTP_BODY_BYTES, "download data")?;
        }
        HostOperation::DownloadPath {
            path,
            filename,
            show_storage_tip,
        } => {
            writer.u8(20);
            writer.path(path)?;
            writer.string(filename, crate::MAX_TEXT_BYTES, "download filename")?;
            writer.bool(*show_storage_tip);
        }
        HostOperation::RevealDownload {
            filename,
            mime_type,
            open_file,
        } => {
            writer.u8(21);
            writer.string(filename, crate::MAX_TEXT_BYTES, "download filename")?;
            writer.string(mime_type, crate::MAX_TEXT_BYTES, "download MIME type")?;
            writer.bool(*open_file);
        }
        HostOperation::CopyImage { mime_type, data } => {
            if !mime_type.starts_with("image/") {
                return Err(EncodeError::InvalidValue("clipboard image MIME type"));
            }
            writer.u8(24);
            writer.string(
                mime_type,
                crate::MAX_TEXT_BYTES,
                "clipboard image MIME type",
            )?;
            writer.bytes(data, crate::MAX_HTTP_BODY_BYTES, "clipboard image data")?;
        }
        HostOperation::SaveAndOpenFile {
            filename,
            mime_type,
            data,
        } => {
            writer.u8(25);
            writer.string(filename, crate::MAX_TEXT_BYTES, "open filename")?;
            writer.string(mime_type, crate::MAX_TEXT_BYTES, "open MIME type")?;
            writer.bytes(data, crate::MAX_HTTP_BODY_BYTES, "open file data")?;
        }
        HostOperation::SetTitle(title) => {
            writer.u8(10);
            writer.string(title, crate::MAX_TEXT_BYTES, "window title")?;
        }
        HostOperation::SetTitlebarVisible(visible) => {
            writer.u8(26);
            writer.bool(*visible);
        }
        HostOperation::MinimizeWindow => writer.u8(27),
        HostOperation::ToggleMaximizeWindow => writer.u8(28),
        HostOperation::StartWindowDrag { x, y } => {
            writer.u8(29);
            writer.i32(*x);
            writer.i32(*y);
        }
        HostOperation::ShowNotification {
            kind,
            title,
            message,
        } => {
            if kind.is_empty() || title.trim().is_empty() {
                return Err(EncodeError::InvalidValue("notification"));
            }
            writer.u8(14);
            writer.string(kind, 32, "notification kind")?;
            writer.string(title, 256, "notification title")?;
            writer.string(message, 1024, "notification message")?;
        }
        HostOperation::OpenExternalUrl(url) => {
            if !url.starts_with("https://") {
                return Err(EncodeError::InvalidValue("external URL"));
            }
            writer.u8(17);
            writer.string(url, 2048, "external URL")?;
        }
        HostOperation::ExtensionStore { payload } => {
            writer.u8(11);
            writer.string(
                payload,
                crate::MAX_TRANSFER_BYTES,
                "extension store request",
            )?;
        }
        HostOperation::CalendarStore { payload } => {
            writer.u8(15);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "calendar store request")?;
        }
        HostOperation::MailStore { payload } => {
            writer.u8(22);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "mail store request")?;
        }
        HostOperation::ContactStore { payload } => {
            writer.u8(23);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "contact store request")?;
        }
        HostOperation::Terminal { session, action } => {
            if *session == 0 {
                return Err(EncodeError::InvalidValue("terminal session"));
            }
            writer.u8(12);
            writer.u64(*session);
            match action {
                TerminalAction::Open { columns, rows } => {
                    validate_terminal_size(*columns, *rows).map_err(EncodeError::InvalidValue)?;
                    writer.u8(0);
                    writer.u16(*columns);
                    writer.u16(*rows);
                }
                TerminalAction::Input(bytes) => {
                    writer.u8(1);
                    writer.bytes(bytes, crate::MAX_TRANSFER_BYTES, "terminal input")?;
                }
                TerminalAction::Resize { columns, rows } => {
                    validate_terminal_size(*columns, *rows).map_err(EncodeError::InvalidValue)?;
                    writer.u8(2);
                    writer.u16(*columns);
                    writer.u16(*rows);
                }
                TerminalAction::Close => writer.u8(3),
                TerminalAction::Configure {
                    copy_on_select,
                    transparency,
                } => {
                    if *transparency > 80 {
                        return Err(EncodeError::InvalidValue("terminal transparency"));
                    }
                    writer.u8(4);
                    writer.u8(u8::from(*copy_on_select));
                    writer.u8(*transparency);
                }
                TerminalAction::SuperFullscreen => writer.u8(5),
                TerminalAction::OpenGrid { columns, rows } => {
                    validate_terminal_size(*columns, *rows).map_err(EncodeError::InvalidValue)?;
                    writer.u8(6);
                    writer.u16(*columns);
                    writer.u16(*rows);
                }
                TerminalAction::OpenXCodex { columns, rows } => {
                    validate_terminal_size(*columns, *rows).map_err(EncodeError::InvalidValue)?;
                    writer.u8(12);
                    writer.u16(*columns);
                    writer.u16(*rows);
                }
                TerminalAction::OpenGridWithMemory { columns, rows, memory_mib } => {
                    validate_terminal_size(*columns, *rows).map_err(EncodeError::InvalidValue)?;
                    if !(384..=2048).contains(memory_mib) {
                        return Err(EncodeError::InvalidValue("terminal memory"));
                    }
                    writer.u8(11);
                    writer.u16(*columns);
                    writer.u16(*rows);
                    writer.u16(*memory_mib);
                }
                TerminalAction::CopyAll => writer.u8(9),
                TerminalAction::Assistant(payload) => {
                    writer.u8(10);
                    writer.string(payload, 16384, "terminal assistant")?;
                }
                TerminalAction::Appearance(payload) => {
                    writer.u8(8);
                    writer.string(payload, 1024, "terminal appearance")?;
                }
                TerminalAction::Scroll(lines) => {
                    writer.u8(7);
                    writer.u32(lines.cast_unsigned());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn decode_request(reader: &mut Reader<'_>) -> Result<HostRequest, DecodeError> {
    let app = reader.id("host request app")?;
    let window = reader.id("host request window")?;
    let operation = match reader.u8()? {
        43 => {
            let token = reader.u64()?; let width = reader.u32()?; let height = reader.u32()?; let anchor = reader.u8()?;
            if anchor > 3 || token == 0 || !(96..=4096).contains(&width) || !(96..=4096).contains(&height) { return Err(DecodeError::InvalidValue("window content size")); }
            HostOperation::SetContentSize { token, width, height, anchor }
        }
        42 => {
            let token=reader.u64()?;
            if token==0 {return Err(DecodeError::InvalidValue("data request fence"));}
            HostOperation::DataRequestFence {token,resume:boolean(reader,"data request resume")?}
        },
        41 => HostOperation::InteractiveNotification {
            payload: reader.string(256 * 1024, "interactive notification")?,
        },
        40 => HostOperation::PaddyWhatsApp {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "Paddy WhatsApp command")?,
        },
        0 => HostOperation::SetFullscreen(boolean(reader, "fullscreen")?),
        1 => {
            let token = reader.u64()?;
            let interval_millis = reader.u32()?;
            let enabled = boolean(reader, "timer enabled")?;
            if token == 0
                || !(MIN_HOST_TIMER_INTERVAL_MILLIS..=86_400_000).contains(&interval_millis)
            {
                return Err(DecodeError::InvalidValue("host timer"));
            }
            HostOperation::SetTimer {
                token,
                interval_millis,
                enabled,
            }
        }
        2 => HostOperation::PrintText {
            title: reader.string(crate::MAX_TEXT_BYTES, "print title")?,
            text: reader.string(crate::MAX_TRANSFER_BYTES, "print text")?,
        },
        3 => HostOperation::RequestClose,
        31 => HostOperation::FileDialogResult {
            path: if boolean(reader, "dialog accepted")? {
                Some(reader.string(crate::MAX_PATH_BYTES, "dialog path")?)
            } else { None },
        },
        37 => HostOperation::MenuResult {
            id: if boolean(reader, "menu selected")? {
                Some(reader.string(256, "menu item ID")?)
            } else { None },
        },
        4 => {
            let session = reader.u64()?;
            if session == 0 {
                return Err(DecodeError::InvalidValue("media session"));
            }
            let command = match reader.u8()? {
                0 => MediaCommand::TogglePause,
                1 => MediaCommand::SeekMillis,
                2 => MediaCommand::SetVolume,
                3 => MediaCommand::ToggleMute,
                4 => MediaCommand::Stop,
                value => {
                    return Err(DecodeError::UnknownTag {
                        field: "media command",
                        value,
                    });
                }
            };
            HostOperation::MediaControl {
                session,
                command,
                value: reader.u64()? as i64,
            }
        }
        18 => {
            let replace_session = reader.u64()?;
            let path = reader.string(crate::MAX_PATH_BYTES, "media path")?;
            validate_media_path(&path).map_err(DecodeError::InvalidValue)?;
            HostOperation::MediaOpen {
                replace_session,
                path,
            }
        }
        38 => {
            let replace_session = reader.u64()?;
            let path = reader.string(crate::MAX_PATH_BYTES, "media path")?;
            validate_media_path(&path).map_err(DecodeError::InvalidValue)?;
            HostOperation::MediaOpenMuted { replace_session, path }
        }
        32 => {
            let request = reader.u64()?;
            let handle = reader.u64()?;
            let action = reader.string(crate::MAX_TEXT_BYTES, "protected media action")?;
            if request == 0 || action.is_empty() {
                return Err(DecodeError::InvalidValue("protected media request"));
            }
            HostOperation::ProtectedMedia {
                request,
                handle,
                action,
                metadata: reader.string(
                    crate::MAX_HTTP_METADATA_BYTES,
                    "protected media metadata",
                )?,
                data: reader.bytes(crate::MAX_HTTP_BODY_BYTES, "protected media data")?,
            }
        }
        35 => {
            let tab = reader.u64()?;
            let realm = reader.u64()?;
            let handle = reader.u64()?;
            let action = reader.u8()?;
            if tab == 0 || realm == 0 || handle == 0 || !(1..=3).contains(&action) {
                return Err(DecodeError::InvalidValue("WebSocket request"));
            }
            HostOperation::WebSocket {
                tab,
                realm,
                handle,
                action,
                metadata: reader.string(32 * 1024, "WebSocket metadata")?,
                data: reader.bytes(1024 * 1024, "WebSocket data")?,
            }
        }
        33 => {
            let tab = reader.u64()?;
            let request = reader.u64()?;
            if tab == 0 || request == 0 {
                return Err(DecodeError::InvalidValue("geolocation request"));
            }
            HostOperation::Geolocation {
                tab,
                request,
                watch: boolean(reader, "geolocation watch")?,
                options: reader.string(1024, "geolocation options")?,
            }
        }
        34 => {
            let tab = reader.u64()?;
            let request = reader.u64()?;
            if tab == 0 || request == 0 {
                return Err(DecodeError::InvalidValue("geolocation watch"));
            }
            HostOperation::ClearGeolocation { tab, request }
        }
        36 => {
            let request = reader.u64()?;
            let tab = reader.u64()?;
            let realm = reader.u64()?;
            let action = reader.string(16, "JavaScript realm action")?;
            if request == 0 || tab == 0 || realm == 0 || action.is_empty() {
                return Err(DecodeError::InvalidValue("JavaScript realm request"));
            }
            HostOperation::JavaScriptRealm {
                request,
                tab,
                realm,
                action,
                url: reader.string(crate::MAX_TEXT_BYTES, "JavaScript realm URL")?,
                source: reader.bytes(32 * 1024 * 1024, "JavaScript realm source")?,
            }
        }
        5 => HostOperation::PerformanceSnapshot,
        13 => HostOperation::AccelerationDiagnostics,
        6 => {
            let pid = reader.u32()?;
            let signal = reader.u8()?;
            if pid == 0 || signal == 0 || signal > 64 {
                return Err(DecodeError::InvalidValue("process signal"));
            }
            HostOperation::ProcessSignal { pid, signal }
        }
        7 => {
            let pid = reader.u32()?;
            let nice = reader.u8()? as i8;
            if pid == 0 || !(-20..=19).contains(&nice) {
                return Err(DecodeError::InvalidValue("process niceness"));
            }
            HostOperation::ProcessNice { pid, nice }
        }
        8 => HostOperation::AppStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "app store request")?,
        },
        9 => HostOperation::DownloadText {
            filename: reader.string(crate::MAX_TEXT_BYTES, "download filename")?,
            mime_type: reader.string(crate::MAX_TEXT_BYTES, "download MIME type")?,
            text: reader.string(crate::MAX_HTTP_BODY_BYTES, "download text")?,
        },
        19 => HostOperation::DownloadFile {
            filename: reader.string(crate::MAX_TEXT_BYTES, "download filename")?,
            mime_type: reader.string(crate::MAX_TEXT_BYTES, "download MIME type")?,
            data: reader.bytes(crate::MAX_HTTP_BODY_BYTES, "download data")?,
        },
        20 => HostOperation::DownloadPath {
            path: reader.path()?,
            filename: reader.string(crate::MAX_TEXT_BYTES, "download filename")?,
            show_storage_tip: reader.bool("storage tip")?,
        },
        21 => HostOperation::RevealDownload {
            filename: reader.string(crate::MAX_TEXT_BYTES, "download filename")?,
            mime_type: reader.string(crate::MAX_TEXT_BYTES, "download MIME type")?,
            open_file: reader.bool("open download")?,
        },
        24 => {
            let mime_type = reader.string(crate::MAX_TEXT_BYTES, "clipboard image MIME type")?;
            if !mime_type.starts_with("image/") {
                return Err(DecodeError::InvalidValue("clipboard image MIME type"));
            }
            HostOperation::CopyImage {
                mime_type,
                data: reader.bytes(crate::MAX_HTTP_BODY_BYTES, "clipboard image data")?,
            }
        }
        25 => HostOperation::SaveAndOpenFile {
            filename: reader.string(crate::MAX_TEXT_BYTES, "open filename")?,
            mime_type: reader.string(crate::MAX_TEXT_BYTES, "open MIME type")?,
            data: reader.bytes(crate::MAX_HTTP_BODY_BYTES, "open file data")?,
        },
        10 => HostOperation::SetTitle(reader.string(crate::MAX_TEXT_BYTES, "window title")?),
        26 => HostOperation::SetTitlebarVisible(boolean(reader, "titlebar visible")?),
        27 => HostOperation::MinimizeWindow,
        28 => HostOperation::ToggleMaximizeWindow,
        29 => HostOperation::StartWindowDrag {
            x: reader.i32()?,
            y: reader.i32()?,
        },
        30 => {
            let topic = reader.string(crate::MAX_TEXT_BYTES, "RiceBus topic")?;
            if topic.is_empty() {
                return Err(DecodeError::InvalidValue("RiceBus topic"));
            }
            let count = reader.count(MAX_RICE_BUS_FILTERS, "RiceBus subscription filters")?;
            let mut filters = Vec::with_capacity(count);
            for _ in 0..count {
                filters.push(reader.string(crate::MAX_PATH_BYTES, "RiceBus subscription filter")?);
            }
            HostOperation::RiceBusSubscribe { topic, filters }
        }
        14 => {
            let kind = reader.string(32, "notification kind")?;
            let title = reader.string(256, "notification title")?;
            let message = reader.string(1024, "notification message")?;
            if kind.is_empty() || title.trim().is_empty() {
                return Err(DecodeError::InvalidValue("notification"));
            }
            HostOperation::ShowNotification {
                kind,
                title,
                message,
            }
        }
        11 => HostOperation::ExtensionStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "extension store request")?,
        },
        15 => HostOperation::CalendarStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "calendar store request")?,
        },
        22 => HostOperation::MailStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "mail store request")?,
        },
        23 => HostOperation::ContactStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "contact store request")?,
        },
        tag @ (16 | 39) => {
            let session = reader.u64()?;
            let action = match reader.u8()? {
                0 => ImageExportAction::Copy,
                1 => ImageExportAction::SaveAs,
                2 => ImageExportAction::SaveTenant,
                value => {
                    return Err(DecodeError::UnknownTag {
                        field: "image export action",
                        value,
                    });
                }
            };
            let width = reader.u16()?;
            let height = reader.u16()?;
            let quarter_turns = reader.u8()?;
            let preserve_original = boolean(reader, "preserve original image")?;
            if session == 0
                || width == 0
                || height == 0
                || width > crate::MAX_MEDIA_DIMENSION
                || height > crate::MAX_MEDIA_DIMENSION
                || quarter_turns > 3
            {
                return Err(DecodeError::InvalidValue("image export"));
            }
            let filename = reader.string(crate::MAX_PATH_BYTES, "image export filename")?;
            let crop = if tag == 39 {
                let [x, y, w, h] = [reader.u16()?, reader.u16()?, reader.u16()?, reader.u16()?];
                if w == 0 || h == 0 || u32::from(x) + u32::from(w) > 4096
                    || u32::from(y) + u32::from(h) > 4096 || preserve_original {
                    return Err(DecodeError::InvalidValue("image crop"));
                }
                Some([x, y, w, h])
            } else { None };
            HostOperation::ImageExport {
                session,
                action,
                width,
                height,
                quarter_turns,
                preserve_original,
                filename,
                crop,
            }
        }
        17 => {
            let url = reader.string(2048, "external URL")?;
            if !url.starts_with("https://") {
                return Err(DecodeError::InvalidValue("external URL"));
            }
            HostOperation::OpenExternalUrl(url)
        }
        12 => {
            let session = reader.u64()?;
            if session == 0 {
                return Err(DecodeError::InvalidValue("terminal session"));
            }
            let action = match reader.u8()? {
                12 => {
                    let columns = reader.u16()?;
                    let rows = reader.u16()?;
                    validate_terminal_size(columns, rows).map_err(DecodeError::InvalidValue)?;
                    TerminalAction::OpenXCodex { columns, rows }
                }
                0 => {
                    let columns = reader.u16()?;
                    let rows = reader.u16()?;
                    validate_terminal_size(columns, rows).map_err(DecodeError::InvalidValue)?;
                    TerminalAction::Open { columns, rows }
                }
                1 => TerminalAction::Input(
                    reader.bytes(crate::MAX_TRANSFER_BYTES, "terminal input")?,
                ),
                2 => {
                    let columns = reader.u16()?;
                    let rows = reader.u16()?;
                    validate_terminal_size(columns, rows).map_err(DecodeError::InvalidValue)?;
                    TerminalAction::Resize { columns, rows }
                }
                3 => TerminalAction::Close,
                4 => {
                    let copy_on_select = boolean(reader, "terminal copy on select")?;
                    let transparency = reader.u8()?;
                    if transparency > 80 {
                        return Err(DecodeError::InvalidValue("terminal transparency"));
                    }
                    TerminalAction::Configure {
                        copy_on_select,
                        transparency,
                    }
                }
                5 => TerminalAction::SuperFullscreen,
                6 => {
                    let columns = reader.u16()?;
                    let rows = reader.u16()?;
                    validate_terminal_size(columns, rows).map_err(DecodeError::InvalidValue)?;
                    TerminalAction::OpenGrid { columns, rows }
                }
                9 => TerminalAction::CopyAll,
                10 => TerminalAction::Assistant(reader.string(16384, "terminal assistant")?),
                11 => {
                    let columns = reader.u16()?;
                    let rows = reader.u16()?;
                    let memory_mib = reader.u16()?;
                    validate_terminal_size(columns, rows).map_err(DecodeError::InvalidValue)?;
                    if !(384..=2048).contains(&memory_mib) {
                        return Err(DecodeError::InvalidValue("terminal memory"));
                    }
                    TerminalAction::OpenGridWithMemory { columns, rows, memory_mib }
                }
                8 => TerminalAction::Appearance(reader.string(1024, "terminal appearance")?),
                7 => TerminalAction::Scroll(reader.u32()?.cast_signed()),
                value => {
                    return Err(DecodeError::UnknownTag {
                        field: "terminal action",
                        value,
                    });
                }
            };
            HostOperation::Terminal { session, action }
        }
        value => {
            return Err(DecodeError::UnknownTag {
                field: "host operation",
                value,
            });
        }
    };
    Ok(HostRequest {
        app,
        window,
        operation,
    })
}

pub(super) fn encode_event(writer: &mut Writer, event: &HostEvent) -> Result<(), EncodeError> {
    writer.id(event.app);
    match &event.event {
        HostEventKind::WindowSizeResult { window, token, width, height, error } => {
            if *token == 0 || !(1..=65535).contains(width) || !(1..=65535).contains(height) { return Err(EncodeError::InvalidValue("window size result")); }
            writer.u8(19); writer.id(*window); writer.u64(*token); writer.u32(*width); writer.u32(*height); writer.string(error, 4096, "window size error")?;
        }
        HostEventKind::DataRequestFence {token,resume,ok} => {
            if *token==0 {return Err(EncodeError::InvalidValue("data request fence"));}
            writer.u8(18);writer.u64(*token);writer.u8(u8::from(*resume));writer.u8(u8::from(*ok));
        }
        HostEventKind::Timer { token } => {
            if *token == 0 {
                return Err(EncodeError::InvalidValue("timer token"));
            }
            writer.u8(0);
            writer.u64(*token);
        }
        HostEventKind::RiceBus { topic, payload } => {
            if topic.is_empty() {
                return Err(EncodeError::InvalidValue("RiceBus topic"));
            }
            writer.u8(12);
            writer.string(topic, crate::MAX_TEXT_BYTES, "RiceBus topic")?;
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "RiceBus event payload")?;
        }
        HostEventKind::WindowState {
            window,
            content_width,
            content_height,
            fullscreen,
            maximized,
            focused,
            visible,
        } => {
            validate_window_size(*content_width, *content_height)
                .map_err(EncodeError::InvalidValue)?;
            writer.u8(1);
            writer.id(*window);
            writer.u32(*content_width);
            writer.u32(*content_height);
            writer.u8(u8::from(*fullscreen));
            writer.u8(u8::from(*maximized));
            writer.u8(u8::from(*focused));
            writer.u8(u8::from(*visible));
        }
        HostEventKind::MediaOpened {
            path,
            session,
            duration_millis,
            error,
        } => {
            if (*session == 0) == error.is_empty() {
                return Err(EncodeError::InvalidValue("media open result"));
            }
            validate_media_path(path).map_err(EncodeError::InvalidValue)?;
            writer.u8(8);
            writer.u64(*session);
            writer.u64(*duration_millis);
            writer.string(path, crate::MAX_PATH_BYTES, "media path")?;
            writer.string(error, crate::MAX_TEXT_BYTES, "media open error")?;
        }
        HostEventKind::MediaEnded { session } => {
            if *session == 0 {
                return Err(EncodeError::InvalidValue("media session"));
            }
            writer.u8(9);
            writer.u64(*session);
        }
        HostEventKind::ProtectedMedia {
            request,
            ok,
            handle,
            event,
            metadata,
            data,
        } => {
            if event.is_empty() {
                return Err(EncodeError::InvalidValue("protected media event"));
            }
            writer.u8(13);
            writer.u64(*request);
            writer.bool(*ok);
            writer.u64(*handle);
            writer.string(event, crate::MAX_TEXT_BYTES, "protected media event")?;
            writer.string(
                metadata,
                crate::MAX_HTTP_METADATA_BYTES,
                "protected media metadata",
            )?;
            writer.bytes(data, crate::MAX_HTTP_BODY_BYTES, "protected media data")?;
        }
        HostEventKind::WebSocket {
            tab,
            realm,
            handle,
            event,
            metadata,
            data,
        } => {
            if *tab == 0 || *realm == 0 || *handle == 0 || event.is_empty() ||
                event.len() > 16 || data.len() > 1024 * 1024
            {
                return Err(EncodeError::InvalidValue("WebSocket event"));
            }
            writer.u8(15);
            writer.u64(*tab);
            writer.u64(*realm);
            writer.u64(*handle);
            writer.string(event, 16, "WebSocket event")?;
            writer.string(metadata, 32 * 1024, "WebSocket metadata")?;
            writer.bytes(data, 1024 * 1024, "WebSocket data")?;
        }
        HostEventKind::Geolocation {
            tab,
            request,
            payload,
        } => {
            if *tab == 0 || *request == 0 {
                return Err(EncodeError::InvalidValue("geolocation result"));
            }
            writer.u8(14);
            writer.u64(*tab);
            writer.u64(*request);
            writer.string(payload, 4096, "geolocation result")?;
        }
        HostEventKind::JavaScriptRealm {
            request,
            tab,
            realm,
            event,
            metadata,
            source,
        } => {
            if *request == 0 || *tab == 0 || *realm == 0 || event.is_empty() {
                return Err(EncodeError::InvalidValue("JavaScript realm event"));
            }
            writer.u8(16);
            writer.u64(*request);
            writer.u64(*tab);
            writer.u64(*realm);
            writer.string(event, 16, "JavaScript realm event")?;
            writer.string(metadata, crate::MAX_HTTP_METADATA_BYTES, "JavaScript realm metadata")?;
            writer.bytes(source, 32 * 1024 * 1024, "JavaScript realm snapshot")?;
        }
        HostEventKind::PerformanceSnapshot { payload } => {
            writer.u8(2);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "performance snapshot")?;
        }
        HostEventKind::AccelerationDiagnostics { payload } => {
            writer.u8(6);
            writer.string(
                payload,
                crate::MAX_TRANSFER_BYTES,
                "acceleration diagnostics",
            )?;
        }
        HostEventKind::AppStore { payload } => {
            writer.u8(3);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "app store event")?;
        }
        HostEventKind::PaddyWhatsApp { payload } => {
            writer.u8(17);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "Paddy WhatsApp event")?;
        }
        HostEventKind::ExtensionStore { payload } => {
            writer.u8(4);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "extension store event")?;
        }
        HostEventKind::CalendarStore { payload } => {
            writer.u8(7);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "calendar store event")?;
        }
        HostEventKind::MailStore { payload } => {
            writer.u8(10);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "mail store event")?;
        }
        HostEventKind::ContactStore { payload } => {
            writer.u8(11);
            writer.string(payload, crate::MAX_TRANSFER_BYTES, "contact store event")?;
        }
        HostEventKind::Terminal { session, event } => {
            if *session == 0 {
                return Err(EncodeError::InvalidValue("terminal session"));
            }
            writer.u8(5);
            writer.u64(*session);
            match event {
                TerminalEvent::Screen(screen) => {
                    validate_terminal_size(screen.columns, screen.rows)
                        .map_err(EncodeError::InvalidValue)?;
                    if usize::try_from(screen.caret).unwrap_or(usize::MAX)
                        > screen.text.chars().count()
                    {
                        return Err(EncodeError::InvalidValue("terminal caret"));
                    }
                    writer.u8(0);
                    writer.u16(screen.columns);
                    writer.u16(screen.rows);
                    writer.u32(screen.caret);
                    writer.u8(u8::from(screen.application_cursor)
                        | (u8::from(screen.application_keypad) << 1)
                        | (u8::from(screen.bracketed_paste) << 2));
                    writer.string(&screen.text, crate::MAX_TRANSFER_BYTES, "terminal screen")?;
                }
                TerminalEvent::GridSize { columns, rows } => {
                    validate_terminal_size(*columns, *rows).map_err(EncodeError::InvalidValue)?;
                    writer.u8(5);
                    writer.u16(*columns);
                    writer.u16(*rows);
                }
                TerminalEvent::Paste(text) => {
                    writer.u8(6);
                    writer.string(text, crate::MAX_TRANSFER_BYTES, "terminal paste")?;
                }
                TerminalEvent::Appearance(payload) => {
                    writer.u8(7);
                    writer.string(payload, 1024, "terminal appearance")?;
                }
                TerminalEvent::Grid(payload) => {
                    writer.u8(4);
                    writer.string(payload, crate::MAX_TERMINAL_GRID_BYTES, "terminal grid")?;
                }
                TerminalEvent::Ready => writer.u8(1),
                TerminalEvent::Exited { code } => {
                    writer.u8(2);
                    writer.u32((*code).cast_unsigned());
                }
                TerminalEvent::Preferences {
                    copy_on_select,
                    transparency,
                } => {
                    if *transparency > 80 {
                        return Err(EncodeError::InvalidValue("terminal transparency"));
                    }
                    writer.u8(3);
                    writer.u8(u8::from(*copy_on_select));
                    writer.u8(*transparency);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn decode_event(reader: &mut Reader<'_>) -> Result<HostEvent, DecodeError> {
    let app = reader.id("host event app")?;
    let event = match reader.u8()? {
        19 => {
            let window = reader.id("window size result window")?; let token = reader.u64()?; let width = reader.u32()?; let height = reader.u32()?;
            if token == 0 || !(1..=65535).contains(&width) || !(1..=65535).contains(&height) { return Err(DecodeError::InvalidValue("window size result")); }
            HostEventKind::WindowSizeResult { window, token, width, height, error:reader.string(4096, "window size error")? }
        }
        18 => {
            let token=reader.u64()?;
            if token==0 {return Err(DecodeError::InvalidValue("data request fence"));}
            HostEventKind::DataRequestFence {token,resume:boolean(reader,"data request resume")?,ok:boolean(reader,"data request fence result")?}
        },
        0 => {
            let token = reader.u64()?;
            if token == 0 {
                return Err(DecodeError::InvalidValue("timer token"));
            }
            HostEventKind::Timer { token }
        }
        12 => {
            let topic = reader.string(crate::MAX_TEXT_BYTES, "RiceBus topic")?;
            if topic.is_empty() {
                return Err(DecodeError::InvalidValue("RiceBus topic"));
            }
            HostEventKind::RiceBus {
                topic,
                payload: reader.string(crate::MAX_TRANSFER_BYTES, "RiceBus event payload")?,
            }
        }
        1 => {
            let window = reader.id("window state window")?;
            let content_width = reader.u32()?;
            let content_height = reader.u32()?;
            validate_window_size(content_width, content_height)
                .map_err(DecodeError::InvalidValue)?;
            HostEventKind::WindowState {
                window,
                content_width,
                content_height,
                fullscreen: boolean(reader, "window fullscreen")?,
                maximized: boolean(reader, "window maximized")?,
                focused: boolean(reader, "window focused")?,
                visible: boolean(reader, "window visible")?,
            }
        }
        8 => {
            let session = reader.u64()?;
            let duration_millis = reader.u64()?;
            let path = reader.string(crate::MAX_PATH_BYTES, "media path")?;
            let error = reader.string(crate::MAX_TEXT_BYTES, "media open error")?;
            if (session == 0) == error.is_empty() {
                return Err(DecodeError::InvalidValue("media open result"));
            }
            validate_media_path(&path).map_err(DecodeError::InvalidValue)?;
            HostEventKind::MediaOpened {
                path,
                session,
                duration_millis,
                error,
            }
        }
        9 => {
            let session = reader.u64()?;
            if session == 0 {
                return Err(DecodeError::InvalidValue("media session"));
            }
            HostEventKind::MediaEnded { session }
        }
        13 => {
            let request = reader.u64()?;
            let ok = boolean(reader, "protected media result")?;
            let handle = reader.u64()?;
            let event = reader.string(crate::MAX_TEXT_BYTES, "protected media event")?;
            if event.is_empty() {
                return Err(DecodeError::InvalidValue("protected media event"));
            }
            HostEventKind::ProtectedMedia {
                request,
                ok,
                handle,
                event,
                metadata: reader.string(
                    crate::MAX_HTTP_METADATA_BYTES,
                    "protected media metadata",
                )?,
                data: reader.bytes(crate::MAX_HTTP_BODY_BYTES, "protected media data")?,
            }
        }
        15 => {
            let tab = reader.u64()?;
            let realm = reader.u64()?;
            let handle = reader.u64()?;
            let event = reader.string(16, "WebSocket event")?;
            if tab == 0 || realm == 0 || handle == 0 || event.is_empty() {
                return Err(DecodeError::InvalidValue("WebSocket event"));
            }
            HostEventKind::WebSocket {
                tab,
                realm,
                handle,
                event,
                metadata: reader.string(32 * 1024, "WebSocket metadata")?,
                data: reader.bytes(1024 * 1024, "WebSocket data")?,
            }
        }
        14 => {
            let tab = reader.u64()?;
            let request = reader.u64()?;
            if tab == 0 || request == 0 {
                return Err(DecodeError::InvalidValue("geolocation result"));
            }
            HostEventKind::Geolocation {
                tab,
                request,
                payload: reader.string(4096, "geolocation result")?,
            }
        }
        16 => {
            let request = reader.u64()?;
            let tab = reader.u64()?;
            let realm = reader.u64()?;
            let event = reader.string(16, "JavaScript realm event")?;
            if request == 0 || tab == 0 || realm == 0 || event.is_empty() {
                return Err(DecodeError::InvalidValue("JavaScript realm event"));
            }
            HostEventKind::JavaScriptRealm {
                request,
                tab,
                realm,
                event,
                metadata: reader.string(
                    crate::MAX_HTTP_METADATA_BYTES,
                    "JavaScript realm metadata",
                )?,
                source: reader.bytes(32 * 1024 * 1024, "JavaScript realm snapshot")?,
            }
        }
        2 => HostEventKind::PerformanceSnapshot {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "performance snapshot")?,
        },
        6 => HostEventKind::AccelerationDiagnostics {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "acceleration diagnostics")?,
        },
        3 => HostEventKind::AppStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "app store event")?,
        },
        17 => HostEventKind::PaddyWhatsApp {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "Paddy WhatsApp event")?,
        },
        4 => HostEventKind::ExtensionStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "extension store event")?,
        },
        7 => HostEventKind::CalendarStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "calendar store event")?,
        },
        10 => HostEventKind::MailStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "mail store event")?,
        },
        11 => HostEventKind::ContactStore {
            payload: reader.string(crate::MAX_TRANSFER_BYTES, "contact store event")?,
        },
        5 => {
            let session = reader.u64()?;
            if session == 0 {
                return Err(DecodeError::InvalidValue("terminal session"));
            }
            let event = match reader.u8()? {
                0 => {
                    let columns = reader.u16()?;
                    let rows = reader.u16()?;
                    validate_terminal_size(columns, rows).map_err(DecodeError::InvalidValue)?;
                    let caret = reader.u32()?;
                    let modes = reader.u8()?;
                    if modes & !0x07 != 0 {
                        return Err(DecodeError::InvalidValue("terminal modes"));
                    }
                    let text = reader.string(crate::MAX_TRANSFER_BYTES, "terminal screen")?;
                    if usize::try_from(caret).unwrap_or(usize::MAX) > text.chars().count() {
                        return Err(DecodeError::InvalidValue("terminal caret"));
                    }
                    TerminalEvent::Screen(crate::TerminalScreen {
                        text,
                        caret,
                        columns,
                        rows,
                        application_cursor: modes & 1 != 0,
                        application_keypad: modes & 2 != 0,
                        bracketed_paste: modes & 4 != 0,
                    })
                }
                5 => {
                    let columns = reader.u16()?;
                    let rows = reader.u16()?;
                    validate_terminal_size(columns, rows).map_err(DecodeError::InvalidValue)?;
                    TerminalEvent::GridSize { columns, rows }
                }
                6 => TerminalEvent::Paste(
                    reader.string(crate::MAX_TRANSFER_BYTES, "terminal paste")?,
                ),
                7 => TerminalEvent::Appearance(reader.string(1024, "terminal appearance")?),
                4 => {
                    TerminalEvent::Grid(reader.string(crate::MAX_TERMINAL_GRID_BYTES, "terminal grid")?)
                }
                1 => TerminalEvent::Ready,
                2 => TerminalEvent::Exited {
                    code: reader.u32()?.cast_signed(),
                },
                3 => {
                    let copy_on_select = boolean(reader, "terminal copy on select")?;
                    let transparency = reader.u8()?;
                    if transparency > 80 {
                        return Err(DecodeError::InvalidValue("terminal transparency"));
                    }
                    TerminalEvent::Preferences {
                        copy_on_select,
                        transparency,
                    }
                }
                value => {
                    return Err(DecodeError::UnknownTag {
                        field: "terminal event",
                        value,
                    });
                }
            };
            HostEventKind::Terminal { session, event }
        }
        value => {
            return Err(DecodeError::UnknownTag {
                field: "host event",
                value,
            });
        }
    };
    Ok(HostEvent { app, event })
}

fn validate_window_size(width: u32, height: u32) -> Result<(), &'static str> {
    const MAX_WINDOW_DIMENSION: u32 = 65_535;
    if width == 0 || height == 0 || width > MAX_WINDOW_DIMENSION || height > MAX_WINDOW_DIMENSION {
        Err("window content size")
    } else {
        Ok(())
    }
}

fn validate_media_path(path: &str) -> Result<(), &'static str> {
    if path.starts_with('/') && !path.contains(['\0', '\n', '\r']) {
        Ok(())
    } else {
        Err("media path")
    }
}

fn validate_terminal_size(columns: u16, rows: u16) -> Result<(), &'static str> {
    if !(2..=512).contains(&columns) || !(1..=256).contains(&rows) {
        Err("terminal size")
    } else {
        Ok(())
    }
}

fn boolean(reader: &mut Reader<'_>, field: &'static str) -> Result<bool, DecodeError> {
    match reader.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(DecodeError::InvalidValue(field)),
    }
}
