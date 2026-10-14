//! Randr operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn randr_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = self.randr_request_inner(handle, order, sequence, minor, body);
        result.map_err(|error| extension_error(error, minor))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn randr_request_inner(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let validate_window = |server: &Self, id: u32| {
            if server.windows.contains_key(&id) {
                Ok(())
            } else {
                Err(ProtocolError::new(x11::error::BAD_WINDOW, id))
            }
        };
        let timestamp = self.timestamp;
        let width = self.config.width_px;
        let height = self.config.height_px;
        let width_mm = u32::from(pixels_to_millimetres(width));
        let height_mm = u32::from(pixels_to_millimetres(height));
        match minor {
            x11randr::RR_QUERY_VERSION => {
                let request = x11randr::parse_query_version(body).ok_or_else(bad_length)?;
                let major = x11randr::MAJOR_VERSION.min(request.major);
                let minor = if major < x11randr::MAJOR_VERSION {
                    request.minor
                } else {
                    x11randr::MINOR_VERSION.min(request.minor)
                };
                Ok(x11randr::encode_query_version_reply(
                    order, sequence, major, minor,
                ))
            }
            x11randr::RR_SET_SCREEN_CONFIG => {
                if body.len() < 20 {
                    return Err(bad_length());
                }
                validate_window(self, read_u32_le(body))?;
                let mut output = fixed_reply_header(order, sequence, 0, 0);
                write_u32_vec(order, &mut output, timestamp);
                write_u32_vec(order, &mut output, timestamp);
                write_u32_vec(order, &mut output, ROOT_WINDOW.0);
                output.resize(32, 0);
                Ok(output)
            }
            x11randr::RR_SELECT_INPUT => {
                let request = x11randr::parse_select_input(body).ok_or_else(bad_length)?;
                validate_window(self, request.window)?;
                let known = x11randr::NOTIFY_MASK_SCREEN_CHANGE
                    | x11randr::NOTIFY_MASK_CRTC_CHANGE
                    | x11randr::NOTIFY_MASK_OUTPUT_CHANGE
                    | x11randr::NOTIFY_MASK_OUTPUT_PROPERTY;
                if request.enable & !known != 0 {
                    return Err(ProtocolError::new(
                        x11::error::BAD_VALUE,
                        u32::from(request.enable),
                    ));
                }
                if request.enable == 0 {
                    self.randr_selects.remove(&(handle.0, request.window));
                } else {
                    self.randr_selects
                        .insert((handle.0, request.window), request.enable);
                }
                Ok(Vec::new())
            }
            x11randr::RR_GET_SCREEN_INFO => {
                let request = x11randr::parse_screen_request(body).ok_or_else(bad_length)?;
                validate_window(self, request.window)?;
                Ok(x11randr::encode_get_screen_info_reply(
                    order,
                    sequence,
                    ROOT_WINDOW.0,
                    timestamp,
                    timestamp,
                    width,
                    height,
                    u16::try_from(width_mm).unwrap_or(u16::MAX),
                    u16::try_from(height_mm).unwrap_or(u16::MAX),
                ))
            }
            x11randr::RR_GET_SCREEN_SIZE_RANGE => {
                let request = x11randr::parse_screen_request(body).ok_or_else(bad_length)?;
                validate_window(self, request.window)?;
                Ok(x11randr::encode_get_screen_size_range_reply(
                    order,
                    sequence,
                    1,
                    1,
                    i16::MAX as u16,
                    i16::MAX as u16,
                ))
            }
            x11randr::RR_SET_SCREEN_SIZE => {
                let request =
                    x11randr::parse_set_screen_size_request(body).ok_or_else(bad_length)?;
                validate_window(self, request.window)?;
                if request.width == 0 || request.height == 0 {
                    return Err(ProtocolError::new(x11::error::BAD_VALUE, 0));
                }
                self.resize_display(request.width, request.height)?;
                Ok(Vec::new())
            }
            x11randr::RR_GET_SCREEN_RESOURCES | x11randr::RR_GET_SCREEN_RESOURCES_CURRENT => {
                let request = x11randr::parse_screen_request(body).ok_or_else(bad_length)?;
                validate_window(self, request.window)?;
                let mode_name = format!("{width}x{height}").into_bytes();
                let mode = x11randr::ModeInfo {
                    id: RANDR_MODE,
                    width,
                    height,
                    dot_clock: u32::from(width)
                        .saturating_mul(u32::from(height))
                        .saturating_mul(60),
                    hsync_start: width,
                    hsync_end: width,
                    htotal: width,
                    hskew: 0,
                    vsync_start: height,
                    vsync_end: height,
                    vtotal: height,
                    name_len: u16::try_from(mode_name.len()).unwrap_or(u16::MAX),
                    mode_flags: 0,
                };
                Ok(x11randr::encode_get_screen_resources_current_reply(
                    order,
                    sequence,
                    &x11randr::ScreenResources {
                        timestamp,
                        config_timestamp: timestamp,
                        crtcs: vec![RANDR_CRTC],
                        outputs: vec![RANDR_OUTPUT],
                        modes: vec![mode],
                        mode_names: mode_name,
                    },
                ))
            }
            x11randr::RR_GET_OUTPUT_INFO => {
                let request = x11randr::parse_output_request(body).ok_or_else(bad_length)?;
                if request.output != RANDR_OUTPUT {
                    return Err(ProtocolError::new(RANDR_FIRST_ERROR, request.output));
                }
                Ok(x11randr::encode_get_output_info_reply(
                    order,
                    sequence,
                    &x11randr::OutputInfoReply {
                        timestamp,
                        crtc: RANDR_CRTC,
                        width_mm,
                        height_mm,
                        connection: x11randr::CONNECTION_CONNECTED,
                        subpixel_order: u8::try_from(x11randr::SUBPIXEL_UNKNOWN)
                            .unwrap_or_default(),
                        crtcs: &[RANDR_CRTC],
                        modes: &[RANDR_MODE],
                        num_preferred: 1,
                        clones: &[],
                        name: b"RiceWM-0",
                    },
                ))
            }
            x11randr::RR_LIST_OUTPUT_PROPERTIES => {
                // ListOutputProperties is the one output-only request in this
                // part of RandR: its body is a single RROutput (4 bytes), not
                // the output + config timestamp pair used by GetOutputInfo.
                // Treating it as OutputRequest rejects every conforming client
                // with BadLength before SDL can create its X11 window.
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let output = read_u32_le(body);
                if output != RANDR_OUTPUT {
                    return Err(ProtocolError::new(RANDR_FIRST_ERROR, output));
                }
                Ok(x11randr::encode_list_output_properties_reply(
                    order,
                    sequence,
                    &[],
                ))
            }
            x11randr::RR_QUERY_OUTPUT_PROPERTY
            | x11randr::RR_CONFIGURE_OUTPUT_PROPERTY
            | x11randr::RR_CHANGE_OUTPUT_PROPERTY
            | x11randr::RR_DELETE_OUTPUT_PROPERTY => {
                let request =
                    x11randr::parse_output_property_request(body).ok_or_else(bad_length)?;
                if request.output == RANDR_OUTPUT {
                    Err(ProtocolError::new(x11::error::BAD_NAME, request.property))
                } else {
                    Err(ProtocolError::new(RANDR_FIRST_ERROR, request.output))
                }
            }
            x11randr::RR_GET_OUTPUT_PROPERTY => {
                let request =
                    x11randr::parse_get_output_property_request(body).ok_or_else(bad_length)?;
                if request.output != RANDR_OUTPUT {
                    return Err(ProtocolError::new(RANDR_FIRST_ERROR, request.output));
                }
                Ok(x11randr::encode_get_output_property_reply(
                    order,
                    sequence,
                    0,
                    0,
                    0,
                    &[],
                ))
            }
            x11randr::RR_GET_CRTC_INFO => {
                let request = x11randr::parse_crtc_request(body).ok_or_else(bad_length)?;
                if request.crtc != RANDR_CRTC {
                    return Err(ProtocolError::new(
                        RANDR_FIRST_ERROR.saturating_add(1),
                        request.crtc,
                    ));
                }
                Ok(x11randr::encode_get_crtc_info_reply(
                    order,
                    sequence,
                    &x11randr::CrtcInfoReply {
                        timestamp,
                        x: 0,
                        y: 0,
                        width,
                        height,
                        mode: RANDR_MODE,
                        rotation: x11randr::ROTATION_ROTATE_0,
                        rotations: x11randr::ROTATION_ROTATE_0,
                        outputs: &[RANDR_OUTPUT],
                        possible: &[RANDR_OUTPUT],
                    },
                ))
            }
            x11randr::RR_SET_CRTC_CONFIG => {
                if body.len() < 24 {
                    return Err(bad_length());
                }
                let crtc = read_u32_le(body);
                if crtc != RANDR_CRTC {
                    return Err(ProtocolError::new(
                        RANDR_FIRST_ERROR.saturating_add(1),
                        crtc,
                    ));
                }
                let mut output = fixed_reply_header(order, sequence, 0, 0);
                write_u32_vec(order, &mut output, timestamp);
                output.resize(32, 0);
                Ok(output)
            }
            x11randr::RR_GET_CRTC_GAMMA_SIZE => {
                let request = x11randr::parse_crtc_id_request(body).ok_or_else(bad_length)?;
                if request.crtc != RANDR_CRTC {
                    return Err(ProtocolError::new(
                        RANDR_FIRST_ERROR.saturating_add(1),
                        request.crtc,
                    ));
                }
                Ok(x11randr::encode_get_crtc_gamma_size_reply(
                    order, sequence, 256,
                ))
            }
            x11randr::RR_GET_CRTC_GAMMA => {
                let request = x11randr::parse_crtc_id_request(body).ok_or_else(bad_length)?;
                if request.crtc != RANDR_CRTC {
                    return Err(ProtocolError::new(
                        RANDR_FIRST_ERROR.saturating_add(1),
                        request.crtc,
                    ));
                }
                let ramp: Vec<u16> = (0..=u8::MAX).map(|entry| u16::from(entry) * 257).collect();
                Ok(x11randr::encode_get_crtc_gamma_reply(
                    order, sequence, &ramp, &ramp, &ramp,
                ))
            }
            x11randr::RR_SET_CRTC_GAMMA => {
                if body.len() < 8 {
                    return Err(bad_length());
                }
                let crtc = read_u32_le(body);
                if crtc == RANDR_CRTC {
                    Ok(Vec::new())
                } else {
                    Err(ProtocolError::new(
                        RANDR_FIRST_ERROR.saturating_add(1),
                        crtc,
                    ))
                }
            }
            x11randr::RR_GET_CRTC_TRANSFORM => {
                let request = x11randr::parse_crtc_id_request(body).ok_or_else(bad_length)?;
                if request.crtc != RANDR_CRTC {
                    return Err(ProtocolError::new(
                        RANDR_FIRST_ERROR.saturating_add(1),
                        request.crtc,
                    ));
                }
                Ok(x11randr::encode_get_crtc_transform_reply(order, sequence))
            }
            x11randr::RR_GET_PANNING => {
                let request = x11randr::parse_crtc_request(body).ok_or_else(bad_length)?;
                if request.crtc != RANDR_CRTC {
                    return Err(ProtocolError::new(
                        RANDR_FIRST_ERROR.saturating_add(1),
                        request.crtc,
                    ));
                }
                Ok(x11randr::encode_get_panning_reply(
                    order, sequence, timestamp,
                ))
            }
            x11randr::RR_SET_OUTPUT_PRIMARY => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                validate_window(self, read_u32_le(body))?;
                let output = read_u32_le(&body[4..8]);
                if output != 0 && output != RANDR_OUTPUT {
                    Err(ProtocolError::new(RANDR_FIRST_ERROR, output))
                } else {
                    Ok(Vec::new())
                }
            }
            x11randr::RR_GET_OUTPUT_PRIMARY => {
                let request = x11randr::parse_screen_request(body).ok_or_else(bad_length)?;
                validate_window(self, request.window)?;
                Ok(x11randr::encode_get_output_primary_reply(
                    order,
                    sequence,
                    RANDR_OUTPUT,
                ))
            }
            x11randr::RR_GET_PROVIDERS => {
                let request = x11randr::parse_screen_request(body).ok_or_else(bad_length)?;
                validate_window(self, request.window)?;
                Ok(x11randr::encode_get_providers_reply(
                    order, sequence, timestamp,
                ))
            }
            x11randr::RR_GET_MONITORS => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                validate_window(self, read_u32_le(body))?;
                let monitor_name = self.atoms.intern("RiceWM-0", false).0;
                Ok(x11randr::encode_get_monitors_reply(
                    order,
                    sequence,
                    timestamp,
                    &[x11randr::MonitorInfo {
                        name: monitor_name,
                        primary: true,
                        automatic: true,
                        x: 0,
                        y: 0,
                        width,
                        height,
                        width_mm,
                        height_mm,
                        outputs: &[RANDR_OUTPUT],
                    }],
                ))
            }
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        }
    }

    pub(super) fn resize_display(&mut self, width: u16, height: u16) -> Result<(), ProtocolError> {
        if width == self.config.width_px && height == self.config.height_px {
            return Ok(());
        }
        self.config.width_px = width;
        self.config.height_px = height;
        self.pointer_x = self.pointer_x.clamp(
            0,
            i16::try_from(width.saturating_sub(1)).unwrap_or(i16::MAX),
        );
        self.pointer_y = self.pointer_y.clamp(
            0,
            i16::try_from(height.saturating_sub(1)).unwrap_or(i16::MAX),
        );
        self.resize_surface(ROOT_WINDOW, width, height)?;
        if let Some(root) = self.windows.get_mut(&ROOT_WINDOW.0) {
            root.width = width;
            root.height = height;
        }
        if let Some(root) = self.windows.get(&ROOT_WINDOW.0).cloned() {
            self.emit_window_event(
                ROOT_WINDOW,
                STRUCTURE_NOTIFY_MASK,
                |output, sequence, order| {
                    x11::encode_configure_notify_event(
                        output,
                        sequence,
                        order,
                        ROOT_WINDOW,
                        ROOT_WINDOW,
                        None,
                        geometry(&root),
                        false,
                    );
                },
            );
        }
        if self.config.native_window_manager {
            let _ = self.resize_surface(WM_GNOME_BACKGROUND, width, height);
            if let Some(background) = self.windows.get_mut(&WM_GNOME_BACKGROUND.0) {
                background.width = width;
                background.height = height;
            }
            self.paint_wm_chrome(WM_GNOME_BACKGROUND);
            self.scene_upsert(WM_GNOME_BACKGROUND);
            self.scene_damage(WM_GNOME_BACKGROUND);
            self.refresh_gnome_panel_geometry();
            self.refresh_taskbar_geometry();
            self.relayout_wm_menu();
            self.relayout_wm_edge_windows();
            let preview_width = WM_TASK_PREVIEW_WIDTH.min(width);
            let preview_height = WM_TASK_PREVIEW_HEIGHT.min(height);
            let _ = self.resize_surface(WM_TASK_PREVIEW, preview_width, preview_height);
            if let Some(preview) = self.windows.get_mut(&WM_TASK_PREVIEW.0) {
                preview.width = preview_width;
                preview.height = preview_height;
            }
            let preview_target = self.wm_task_preview_target.take();
            self.set_wm_task_preview(preview_target, self.pointer_x);
            self.update_wm_quick_launch_tooltip();
            let (notification_x, notification_y, notification_width, notification_height) =
                self.wm_notification_geometry();
            let _ = self.resize_surface(WM_NOTIFICATION, notification_width, notification_height);
            if let Some(notification) = self.windows.get_mut(&WM_NOTIFICATION.0) {
                notification.x = notification_x;
                notification.y = notification_y;
                notification.width = notification_width;
                notification.height = notification_height;
            }
            self.paint_wm_chrome(WM_NOTIFICATION);
            self.scene_upsert(WM_NOTIFICATION);
            self.scene_damage(WM_NOTIFICATION);
            self.publish_wm_root_properties();
            self.apply_workspace_visibility();
        }
        let timestamp = self.next_timestamp();
        let width_mm = pixels_to_millimetres(width);
        let height_mm = pixels_to_millimetres(height);
        let subscribers = self
            .randr_selects
            .iter()
            .filter_map(|(&(owner, request_window), &mask)| {
                (mask & x11randr::NOTIFY_MASK_SCREEN_CHANGE != 0)
                    .then_some((ClientHandle(owner), request_window))
            })
            .collect::<Vec<_>>();
        for (owner, request_window) in subscribers {
            self.emit_to_client(owner, |output, sequence, order| {
                output.extend_from_slice(&x11randr::encode_screen_change_notify_event(
                    order,
                    RANDR_FIRST_EVENT,
                    sequence,
                    x11randr::ScreenChangeNotify {
                        timestamp,
                        config_timestamp: timestamp,
                        root: ROOT_WINDOW.0,
                        request_window,
                        width,
                        height,
                        width_mm,
                        height_mm,
                    },
                ));
            });
        }
        self.scene_upsert(ROOT_WINDOW);
        self.scene_damage(ROOT_WINDOW);
        Ok(())
    }
}
