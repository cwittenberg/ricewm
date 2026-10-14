//! Properties operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn set_wm_property(
        &mut self,
        window: ResourceId,
        name: &str,
        type_name: &str,
        format: u8,
        data: Vec<u8>,
    ) {
        let property = self.atoms.intern(name, false);
        let r#type = self.atoms.intern(type_name, false);
        let changed = if let Some(window) = self.windows.get_mut(&window.0) {
            window.properties.insert(
                property,
                Property {
                    r#type,
                    format,
                    data,
                },
            );
            true
        } else {
            false
        };
        if changed {
            let timestamp = self.next_timestamp();
            self.emit_window_event(window, PROPERTY_CHANGE_MASK, |output, sequence, order| {
                x11::encode_property_notify_event(
                    output, sequence, order, window, property, timestamp, false,
                );
            });
        }
    }

    pub(super) fn set_wm_property_if_changed(
        &mut self,
        window: ResourceId,
        name: &str,
        type_name: &str,
        format: u8,
        data: Vec<u8>,
    ) {
        let property = self.atoms.intern(name, false);
        let r#type = self.atoms.intern(type_name, false);
        let unchanged = self
            .windows
            .get(&window.0)
            .and_then(|window| window.properties.get(&property))
            .is_some_and(|existing| {
                existing.r#type == r#type && existing.format == format && existing.data == data
            });
        if !unchanged {
            self.set_wm_property(window, name, type_name, format, data);
        }
    }

    pub(super) fn publish_gtk_theme_settings(&mut self) {
        let Some(palette) = self.wm_themes.get(usize::from(self.wm_theme)).cloned() else {
            return;
        };
        self.wm_xsettings_serial = self.wm_xsettings_serial.wrapping_add(1).max(1);
        let selection = self.atoms.intern("_XSETTINGS_S0", false);
        let timestamp = self.next_timestamp();
        self.selections
            .insert(selection.0, (WM_XSETTINGS, timestamp, u32::MAX));
        self.set_wm_property(
            WM_XSETTINGS,
            "_XSETTINGS_SETTINGS",
            "_XSETTINGS_SETTINGS",
            8,
            xsettings_theme_payload(self.wm_xsettings_serial, self.wm_theme, &palette),
        );
    }

    pub(super) fn publish_wm_root_properties(&mut self) {
        let supported_names = [
            "_NET_SUPPORTED",
            "_NET_SUPPORTING_WM_CHECK",
            "_NET_WM_NAME",
            "_NET_CLIENT_LIST",
            "_NET_CLIENT_LIST_STACKING",
            "_NET_ACTIVE_WINDOW",
            "_NET_NUMBER_OF_DESKTOPS",
            "_NET_CURRENT_DESKTOP",
            "_NET_DESKTOP_NAMES",
            "_NET_WORKAREA",
            "_NET_WM_DESKTOP",
            "_NET_WM_STATE",
            "_NET_WM_STATE_FULLSCREEN",
            "_NET_WM_STATE_MAXIMIZED_VERT",
            "_NET_WM_STATE_MAXIMIZED_HORZ",
            "_NET_WM_STATE_HIDDEN",
            "_NET_CLOSE_WINDOW",
            "_NET_MOVERESIZE_WINDOW",
            "_NET_WM_MOVERESIZE",
            "_NET_REQUEST_FRAME_EXTENTS",
            "_NET_FRAME_EXTENTS",
            "_NET_WM_WINDOW_TYPE",
            "_NET_WM_WINDOW_TYPE_NORMAL",
            "_NET_WM_WINDOW_TYPE_DIALOG",
            "_NET_WM_WINDOW_TYPE_DOCK",
            "_NET_WM_WINDOW_TYPE_DESKTOP",
            "_NET_WM_PID",
            "_NET_WM_ICON",
            "WM_STATE",
        ];
        let mut supported = Vec::with_capacity(supported_names.len() * 4);
        for name in supported_names {
            supported.extend_from_slice(&self.atoms.intern(name, false).0.to_le_bytes());
        }
        self.set_wm_property_if_changed(ROOT_WINDOW, "_NET_SUPPORTED", "ATOM", 32, supported);
        self.set_wm_property_if_changed(
            ROOT_WINDOW,
            "_NET_SUPPORTING_WM_CHECK",
            "WINDOW",
            32,
            WM_CHECK.0.to_le_bytes().to_vec(),
        );
        self.set_wm_property_if_changed(
            WM_CHECK,
            "_NET_SUPPORTING_WM_CHECK",
            "WINDOW",
            32,
            WM_CHECK.0.to_le_bytes().to_vec(),
        );
        self.set_wm_property_if_changed(
            WM_CHECK,
            "_NET_WM_NAME",
            "UTF8_STRING",
            8,
            b"RiceWM".to_vec(),
        );
        self.set_wm_property_if_changed(
            ROOT_WINDOW,
            "_NET_NUMBER_OF_DESKTOPS",
            "CARDINAL",
            32,
            self.workspace_count.to_le_bytes().to_vec(),
        );
        self.set_wm_property_if_changed(
            ROOT_WINDOW,
            "_NET_CURRENT_DESKTOP",
            "CARDINAL",
            32,
            self.current_workspace.to_le_bytes().to_vec(),
        );
        self.set_wm_property_if_changed(
            ROOT_WINDOW,
            "_NET_DESKTOP_NAMES",
            "UTF8_STRING",
            8,
            b"1\0Work\0Web\0Files\0".to_vec(),
        );
        let (work_x, work_y, work_width, work_height) = self.wm_workarea_rect();
        let mut workarea = Vec::new();
        for _ in 0..self.workspace_count {
            for value in [
                u32::from(work_x),
                u32::from(work_y),
                u32::from(work_width),
                u32::from(work_height),
            ] {
                workarea.extend_from_slice(&value.to_le_bytes());
            }
        }
        self.set_wm_property_if_changed(ROOT_WINDOW, "_NET_WORKAREA", "CARDINAL", 32, workarea);
        self.update_wm_client_properties();
    }

    pub(super) fn update_wm_client_properties(&mut self) {
        let clients: Vec<ResourceId> = self
            .managed_frames
            .keys()
            .copied()
            .map(ResourceId)
            .collect();
        let mut data = Vec::with_capacity(clients.len() * 4);
        for client in &clients {
            data.extend_from_slice(&client.0.to_le_bytes());
        }
        self.set_wm_property_if_changed(
            ROOT_WINDOW,
            "_NET_CLIENT_LIST",
            "WINDOW",
            32,
            data.clone(),
        );
        self.set_wm_property_if_changed(
            ROOT_WINDOW,
            "_NET_CLIENT_LIST_STACKING",
            "WINDOW",
            32,
            data,
        );
        if self.config.native_window_manager && self.surfaces.contains_key(&WM_TASKBAR.0) {
            self.paint_wm_chrome(WM_TASKBAR);
            self.scene_upsert(WM_TASKBAR);
        }
    }
}
