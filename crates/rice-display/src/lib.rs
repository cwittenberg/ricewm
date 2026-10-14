use std::{
    slice,
    sync::{Mutex, OnceLock},
};

use rice_tray::{LauncherTarget, MAX_TRAY_ITEMS, TrayIcon, TrayItem, TrayLauncher};
use rice_x11_core::{
    ClientHandle, DisplayConfig, DisplayServer, WmGnomeBackgroundText, WmGnomeIndicator,
    WmGnomeMenuItem, WmGnomeOverlayRect, WmNativeTask, WmQuickLauncher,
};

pub mod native_ui;
pub mod native_ui_abi;
mod native_ui_snapshot;
mod native_ui_theme;
mod native_fullscreen;

const ABI_VERSION: u32 = 2;
const RESULT_ERROR: u32 = u32::MAX;
const MAX_IO_BUFFER_BYTES: usize = 16 * 1024 * 1024;

static SERVER: OnceLock<Mutex<Option<DisplayServer>>> = OnceLock::new();
static IO_BUFFER: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

fn server_slot() -> &'static Mutex<Option<DisplayServer>> {
    SERVER.get_or_init(|| Mutex::new(None))
}

fn io_buffer() -> &'static Mutex<Vec<u8>> {
    IO_BUFFER.get_or_init(|| Mutex::new(Vec::new()))
}

fn with_server<T>(operation: impl FnOnce(&mut DisplayServer) -> T) -> Option<T> {
    let mut slot = server_slot().lock().ok()?;
    slot.as_mut().map(operation)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_timestamp_millis(milliseconds: u32) -> u32 {
    with_server(|server| {
        server.set_timestamp_millis(milliseconds);
        1
    }).unwrap_or(0)
}

/// Reserve a stable browser-visible staging region.
///
/// The pointer remains valid until the next `rice_io_reserve` call. `RiceWM`'s
/// initial browser adapter is single-Worker; shared rings will replace this
/// staging API when concurrent transport is introduced.
#[unsafe(no_mangle)]
pub extern "C" fn rice_io_reserve(capacity: u32) -> *mut u8 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return std::ptr::null_mut();
    };
    if capacity == 0 || capacity > MAX_IO_BUFFER_BYTES {
        return std::ptr::null_mut();
    }
    let Ok(mut buffer) = io_buffer().lock() else {
        return std::ptr::null_mut();
    };
    buffer.resize(capacity, 0);
    buffer.as_mut_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_io_capacity() -> u32 {
    io_buffer()
        .lock()
        .ok()
        .and_then(|buffer| u32::try_from(buffer.len()).ok())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_init(width_px: u32, height_px: u32) -> u32 {
    let Ok(width_px) = u16::try_from(width_px) else {
        return 0;
    };
    let Ok(height_px) = u16::try_from(height_px) else {
        return 0;
    };
    if width_px == 0 || height_px == 0 {
        return 0;
    }

    let Ok(mut slot) = server_slot().lock() else {
        return 0;
    };
    *slot = Some(DisplayServer::new(DisplayConfig {
        width_px,
        height_px,
        native_window_manager: true,
        ..DisplayConfig::default()
    }));
    1
}

/// Resize the browser-owned native output without restarting X11 clients.
#[unsafe(no_mangle)]
pub extern "C" fn rice_resize(width_px: u32, height_px: u32) -> u32 {
    let (Ok(width_px), Ok(height_px)) = (u16::try_from(width_px), u16::try_from(height_px)) else {
        return 0;
    };
    with_server(|server| u32::from(server.resize_output(width_px, height_px))).unwrap_or(0)
}

/// Atomically update the desktop output layout from bounded u16 LE rectangles.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_shell_monitors_set(topbar: u32, dock: u32) -> u32 {
    if topbar > 1 || dock > 1 { return 0; }
    with_server(|server| u32::from(server.set_shell_monitors(topbar != 0, dock != 0))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_shell_output_select(x: i32, y: i32) -> u32 {
    let (Ok(x),Ok(y)) = (i16::try_from(x),i16::try_from(y)) else { return 0; };
    with_server(|server|u32::from(server.select_shell_output(x,y))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_drag_output_select(source:u32,x:i32,y:i32)->u32 {
    let (Ok(x),Ok(y))=(i16::try_from(x),i16::try_from(y)) else{return 0;};
    with_server(|server|u32::from(server.select_drag_output(source,x,y))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_output_layout(width: u32, height: u32, count: u32) -> u32 {
    let (Ok(width),Ok(height))=(u16::try_from(width),u16::try_from(height)) else {return 0;};
    if count>16 {return 0;}
    let Ok(buffer)=io_buffer().lock() else {return 0;};
    let Some(bytes)=buffer.get(..count as usize*8) else {return 0;};
    let outputs=bytes.chunks_exact(8).map(|b|(
        u16::from_le_bytes([b[0],b[1]]),u16::from_le_bytes([b[2],b[3]]),
        u16::from_le_bytes([b[4],b[5]]),u16::from_le_bytes([b[6],b[7]])
    )).collect();
    with_server(|server|u32::from(server.set_output_layout(width,height,outputs))).unwrap_or(0)
}

/// Records contain accepted geometry followed by the previous inventory index.
#[unsafe(no_mangle)]
pub extern "C" fn rice_output_layout_reorder(width: u32, height: u32, count: u32) -> u32 {
    let (Ok(width),Ok(height))=(u16::try_from(width),u16::try_from(height)) else {return 0;};
    if count == 0 || count > 16 {return 0;}
    let Ok(buffer)=io_buffer().lock() else {return 0;};
    let Some(bytes)=buffer.get(..count as usize*12) else {return 0;};
    let mut outputs=Vec::new();let mut indices=Vec::new();
    for b in bytes.chunks_exact(12) {
        outputs.push((u16::from_le_bytes([b[0],b[1]]),u16::from_le_bytes([b[2],b[3]]),u16::from_le_bytes([b[4],b[5]]),u16::from_le_bytes([b[6],b[7]])));
        indices.push(u32::from_le_bytes([b[8],b[9],b[10],b[11]]));
    }
    with_server(|server|u32::from(server.reorder_output_layout(width,height,outputs,&indices))).unwrap_or(0)
}

/// A smaller named inventory retains each surviving previous output index.
#[unsafe(no_mangle)]
pub extern "C" fn rice_output_layout_subset(width: u32, height: u32, count: u32) -> u32 {
    let (Ok(width),Ok(height))=(u16::try_from(width),u16::try_from(height)) else {return 0;};
    if count == 0 || count > 16 {return 0;}
    let Ok(buffer)=io_buffer().lock() else {return 0;};
    let Some(bytes)=buffer.get(..count as usize*12) else {return 0;};
    let mut outputs=Vec::new();let mut indices=Vec::new();
    for b in bytes.chunks_exact(12) {
        outputs.push((u16::from_le_bytes([b[0],b[1]]),u16::from_le_bytes([b[2],b[3]]),u16::from_le_bytes([b[4],b[5]]),u16::from_le_bytes([b[6],b[7]])));
        indices.push(u32::from_le_bytes([b[8],b[9],b[10],b[11]]));
    }
    with_server(|server|u32::from(server.subset_output_layout(width,height,outputs,&indices))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_output_layout_map(width:u32,height:u32,count:u32)->u32 {
    let (Ok(width),Ok(height))=(u16::try_from(width),u16::try_from(height)) else{return 0;};
    if count==0 || count>16{return 0;}
    let Ok(buffer)=io_buffer().lock() else{return 0;};
    let Some(bytes)=buffer.get(..68+count as usize*8) else{return 0;};
    let mapping:Vec<u32>=bytes[..64].chunks_exact(4).map(|b|u32::from_le_bytes([b[0],b[1],b[2],b[3]])).collect();
    let reset_mask=u32::from_le_bytes([bytes[64],bytes[65],bytes[66],bytes[67]]);
    let outputs=bytes[68..].chunks_exact(8).map(|b|(u16::from_le_bytes([b[0],b[1]]),u16::from_le_bytes([b[2],b[3]]),u16::from_le_bytes([b[4],b[5]]),u16::from_le_bytes([b[6],b[7]]))).collect();
    with_server(|server|u32::from(server.map_output_layout(width,height,outputs,&mapping,reset_mask))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_client_open() -> u32 {
    with_server(|server| server.open_client().map_or(0, |client| client.0)).unwrap_or(0)
}

/// Feed a byte batch from one X11 stream.
///
/// Returns the number of consumed bytes or `u32::MAX` for an invalid client,
/// protocol fault, or queue-limit violation.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must point to a readable region of
/// at least `input_len` bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_client_feed(
    client: u32,
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    if input_len != 0 && input_ptr.is_null() {
        return RESULT_ERROR;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return RESULT_ERROR;
    };
    let input = if input_len == 0 {
        &[]
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    with_server(|server| {
        server
            .feed(ClientHandle(client), input)
            .ok()
            .and_then(|consumed| u32::try_from(consumed).ok())
            .unwrap_or(RESULT_ERROR)
    })
    .unwrap_or(RESULT_ERROR)
}

/// Drain queued X11 replies, errors, and events.
///
/// Returns the number of written bytes or `u32::MAX` for an invalid client.
///
/// # Safety
///
/// When `output_capacity` is nonzero, `output_ptr` must point to a writable
/// region of at least `output_capacity` bytes in this Wasm instance's linear
/// memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_client_drain(
    client: u32,
    output_ptr: *mut u8,
    output_capacity: u32,
) -> u32 {
    if output_capacity != 0 && output_ptr.is_null() {
        return RESULT_ERROR;
    }
    let Ok(output_capacity) = usize::try_from(output_capacity) else {
        return RESULT_ERROR;
    };
    let output = if output_capacity == 0 {
        &mut []
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts_mut(output_ptr, output_capacity) }
    };
    with_server(|server| {
        server
            .drain(ClientHandle(client), output)
            .ok()
            .and_then(|written| u32::try_from(written).ok())
            .unwrap_or(RESULT_ERROR)
    })
    .unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_client_pending(client: u32) -> u32 {
    with_server(|server| {
        server
            .pending_output(ClientHandle(client))
            .ok()
            .and_then(|pending| u32::try_from(pending).ok())
            .unwrap_or(RESULT_ERROR)
    })
    .unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_client_status(client: u32) -> u32 {
    with_server(|server| {
        server
            .status(ClientHandle(client))
            .map_or(0, |status| status as u32)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_client_close(client: u32) -> u32 {
    with_server(|server| u32::from(server.close_client(ClientHandle(client)))).unwrap_or(0)
}

/// Classify the visually topmost X11 pointer target.
///
/// Returns 1 for the native desktop/root layer, 2 for an X11 application
/// window, 3 for RiceWM shell chrome, and 0 for invalid coordinates or an
/// unavailable display.
#[unsafe(no_mangle)]
pub extern "C" fn rice_input_pointer_target(root_x: i32, root_y: i32) -> u32 {
    let (Ok(root_x), Ok(root_y)) = (i16::try_from(root_x), i16::try_from(root_y)) else {
        return 0;
    };
    with_server(|server| u32::from(server.pointer_layer(root_x, root_y)) + 1).unwrap_or(0)
}

/// Return the semantic cursor requested by the X11 window beneath the pointer.
#[unsafe(no_mangle)]
pub extern "C" fn rice_input_pointer_cursor() -> u32 {
    with_server(|server| server.pointer_cursor()).unwrap_or(0)
}

/// Inject pointer motion or a button transition from the browser.
///
/// `kind` is 0 for motion, 1 for button press, and 2 for button release.
/// Detail 6 is the browser-native double-click gesture consumed by `RiceWM`.
#[unsafe(no_mangle)]
pub extern "C" fn rice_input_pointer(
    kind: u32,
    detail: u32,
    root_x: i32,
    root_y: i32,
    state: u32,
) -> u32 {
    let (Ok(detail), Ok(root_x), Ok(root_y), Ok(state)) = (
        u8::try_from(detail),
        i16::try_from(root_x),
        i16::try_from(root_y),
        u16::try_from(state),
    ) else {
        return 0;
    };
    with_server(|server| match kind {
        0 if detail == 0 => {
            server.inject_pointer_motion(root_x, root_y, state);
            1
        }
        1 | 2 if (1..=6).contains(&detail) => {
            server.inject_pointer_button(kind == 1, detail, root_x, root_y, state);
            1
        }
        _ => 0,
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_input_relative_capture() -> u32 {
    with_server(|server| u32::from(server.relative_pointer_capture_requested())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_input_pointer_relative(x: i32, y: i32, dx: i32, dy: i32, state: u32) -> u32 {
    let (Ok(x), Ok(y), Ok(state)) = (i16::try_from(x), i16::try_from(y), u16::try_from(state)) else { return 0; };
    with_server(|server| {
        server.inject_pointer_motion_with_delta(x, y, dx, dy, state);
        1
    }).unwrap_or(0)
}

/// Inject a key transition from the browser using the X11 keycode range.
#[unsafe(no_mangle)]
pub extern "C" fn rice_input_key(pressed: u32, keycode: u32, state: u32) -> u32 {
    let (Ok(keycode), Ok(state)) = (u8::try_from(keycode), u16::try_from(state)) else {
        return 0;
    };
    if pressed > 1 || keycode < 8 {
        return 0;
    }
    with_server(|server| {
        server.inject_key(pressed == 1, keycode, state);
        1
    })
    .unwrap_or(0)
}

/// Update the native taskbar clock using the browser's local wall clock.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_set_clock(hours: u32, minutes: u32) -> u32 {
    let (Ok(hours), Ok(minutes)) = (u8::try_from(hours), u8::try_from(minutes)) else {
        return 0;
    };
    with_server(|server| u32::from(server.set_wall_clock(hours, minutes))).unwrap_or(0)
}

/// Set the active external connectivity strategy: offline=0, hosted=1,
/// local gateway=2, or remote proxy=3.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_connectivity_set(mode: u32) -> u32 {
    let Ok(mode) = u8::try_from(mode) else {
        return 0;
    };
    with_server(|server| u32::from(server.set_connectivity_mode(mode))).unwrap_or(0)
}

/// Enable the native OS status/power cluster and publish its current state.
/// This remains disabled unless the dedicated desktop adapter opts in.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_system_controls_set(state: u32) -> u32 {
    let Ok(state)=u8::try_from(state) else{return 0;};
    with_server(|server|u32::from(server.set_system_controls(state))).unwrap_or(0)
}

/// Set the laptop battery percentage shown by the native system cluster.
/// `255` means that no battery exists and removes the slot.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_system_battery_set(percentage: u32) -> u32 {
    let value=if percentage==255 {None}else{u8::try_from(percentage).ok()};
    if percentage!=255&&value.is_none(){return 0;}
    with_server(|server|u32::from(server.set_system_battery(value))).unwrap_or(0)
}

/// Retain the Rice Box top-bar shortcut in the compositor-owned panel.
/// `u32::MAX` disables it for targets which do not opt in; otherwise the
/// value is the bounded unread count displayed beside the envelope.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_mail_indicator_set(unread: u32) -> u32 {
    let value = if unread == u32::MAX {
        None
    } else {
        u16::try_from(unread).ok()
    };
    if unread != u32::MAX && value.is_none() {
        return 0;
    }
    with_server(|server| u32::from(server.set_mail_indicator(value))).unwrap_or(0)
}

/// Select the bounded native taskbar presentation: panel (0) or floating dock (1).
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_dock_hover_clear() {
    with_server(|server| server.clear_dock_hover());
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_dock_identity_set(kind: u32, id: u32, key: u32) -> u32 {
    with_server(|server| u32::from(server.set_dock_identity(kind,id,key))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_dock_appearance_set(launcher: u32, monochrome: u32, color: u32) -> u32 {
    if launcher > 1 || monochrome > 1 || color > 0xffffff { return 0; }
    with_server(|server| u32::from(server.set_dock_appearance(launcher == 1, (monochrome == 1).then_some(color)))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_shell_style_set(style: u32) -> u32 {
    if style > 1 { return 0; }
    with_server(|server| u32::from(server.set_embedded_shell(style == 1))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_dock_style_set(style: u32) -> u32 {
    if style > 1 { return 0; }
    with_server(|server| u32::from(server.set_dock_style(style == 1))).unwrap_or(0)
}

/// Change both shell menu painting and pointer row geometry.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_theme_menu_row_height_set(height:u32)->u32 {
    with_server(|server|u32::from(server.set_theme_menu_row_height(height as usize))).unwrap_or(0)
}

/// Apply the selected theme's corner geometry to the compact dock.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_dock_corner_radius_set(radius: u32) -> u32 {
    if radius > 32 { return 0; }
    with_server(|server| u32::from(server.set_dock_corner_radius(radius as usize))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_dock_height_set(height: u32) -> u32 {
    if !(32..=80).contains(&height) { return 0; }
    with_server(|server| u32::from(server.set_dock_height(height as usize))).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_dock_magnification_set(enabled: u32, zoom: u32, spread: u32) -> u32 {
    if enabled > 1 || zoom > 100 || spread > 100 {
        return 0;
    }
    with_server(|server| {
        u32::from(server.set_dock_magnification(enabled == 1, zoom as u8, spread as u8))
    })
    .unwrap_or(0)
}

/// Apply RiceSettings taskbar behavior directly to the native window manager.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_taskbar_configure(visible: u32, edge: u32, auto_hide: u32) -> u32 {
    if visible > 1 || edge > 3 || auto_hide > 1 {
        return 0;
    }
    with_server(|server| {
        u32::from(server.set_taskbar_configuration(visible == 1, edge as u8, auto_hide == 1))
    })
    .unwrap_or(0)
}

/// Configure the root RiceWM menu. Every argument is a boolean integer.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_menu_layout_configure(
    applications_only: u32,
    show_themes: u32,
    show_workspaces: u32,
) -> u32 {
    if applications_only > 1 || show_themes > 1 || show_workspaces > 1 {
        return 0;
    }
    with_server(|server| {
        u32::from(server.set_menu_layout(
            applications_only == 1,
            show_themes == 1,
            show_workspaces == 1,
        ))
    })
    .unwrap_or(0)
}

/// Select hosted-session wording for the existing exit Command.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_session_logout_set(enabled: u32) -> u32 {
    if enabled > 1 {
        return 0;
    }
    with_server(|server| u32::from(server.set_session_logout(enabled == 1))).unwrap_or(0)
}

/// Set the exact menu icon edge in CSS pixels.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_menu_icon_size_set(size: u32) -> u32 {
    let Ok(size) = u8::try_from(size) else {
        return 0;
    };
    with_server(|server| u32::from(server.set_menu_icon_size(size))).unwrap_or(0)
}

/// Bridge GNOME overview dash/workspace visibility to RiceWM's existing
/// taskbar elements. Values must be boolean integers.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_gnome_overview_elements_set(
    dash_visible: u32,
    workspaces_visible: u32,
) -> u32 {
    if dash_visible > 1 || workspaces_visible > 1 {
        return 0;
    }
    with_server(|server| {
        u32::from(server.set_gnome_overview_elements(dash_visible == 1, workspaces_visible == 1))
    })
    .unwrap_or(0)
}

/// Apply a translated GNOME Shell 50 panel snapshot.
///
/// `enabled`, `indicator_visible`, and `indicator_has_icon` are booleans. The
/// optional label is bounded UTF-8 copied from the browser staging buffer.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must address its declared number
/// of readable bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_panel_update(
    enabled: u32,
    indicator_visible: u32,
    indicator_has_icon: u32,
    indicator_position: u32,
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    if enabled > 1 || indicator_visible > 1 || indicator_has_icon > 1 || indicator_position > 2 {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_len > 128 || (input_len != 0 && input_ptr.is_null()) {
        return 0;
    }
    let label = if input_len == 0 {
        Vec::new()
    } else {
        // SAFETY: the caller contract above supplies readable Wasm memory.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }.to_vec()
    };
    with_server(|server| {
        u32::from(server.update_gnome_panel(
            enabled == 1,
            vec![WmGnomeIndicator {
                uuid: b"legacy".to_vec(),
                visible: indicator_visible == 1,
                has_icon: indicator_has_icon == 1,
                position: u8::try_from(indicator_position).unwrap_or(2),
                index: 0,
                label,
                image: None,
            }],
        ))
    })
    .unwrap_or(0)
}

/// Apply a translated GNOME Shell 50 panel snapshot for any number of
/// enabled extensions. The bounded record stream is:
/// `u32 count`, then per indicator `u32 uuid_len, uuid, u32 label_len,
/// label, u8 visible, u8 has_icon, u8 position, u8 index`.
///
/// # Safety
///
/// `input_ptr` must address `input_len` readable bytes when `input_len` is
/// nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_panel_update_v2(
    enabled: u32,
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if enabled > 1 {
        return 0;
    }
    if input_len == 0 {
        return with_server(|server| {
            u32::from(server.update_gnome_panel(enabled == 1, Vec::new()))
        })
        .unwrap_or(0);
    }
    if input_ptr.is_null() || input_len > 4096 {
        return 0;
    }
    // SAFETY: the caller contract above supplies readable Wasm memory.
    let bytes = unsafe { slice::from_raw_parts(input_ptr, input_len) }.to_vec();
    if bytes.len() < 4 {
        return 0;
    }
    let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let mut cursor = 4usize;
    if count > 16 {
        return 0;
    }
    let mut indicators = Vec::with_capacity(count);
    for _ in 0..count {
        let read_u32 = |cursor: &mut usize| -> Option<u32> {
            if *cursor + 4 > bytes.len() {
                return None;
            }
            let value = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().ok()?);
            *cursor += 4;
            Some(value)
        };
        let Some(uuid_len) = read_u32(&mut cursor) else {
            return 0;
        };
        let uuid_len = uuid_len as usize;
        if uuid_len == 0 || uuid_len > 128 || cursor + uuid_len > bytes.len() {
            return 0;
        }
        let uuid = bytes[cursor..cursor + uuid_len].to_vec();
        cursor += uuid_len;
        let Some(label_len) = read_u32(&mut cursor) else {
            return 0;
        };
        let label_len = label_len as usize;
        if label_len > 128 || cursor + label_len > bytes.len() {
            return 0;
        }
        let label = bytes[cursor..cursor + label_len].to_vec();
        cursor += label_len;
        if cursor + 4 > bytes.len() {
            return 0;
        }
        let visible = bytes[cursor] == 1;
        let has_icon = bytes[cursor + 1] == 1;
        let position = bytes[cursor + 2];
        let index = bytes[cursor + 3];
        cursor += 4;
        if position > 2 {
            return 0;
        }
        indicators.push(WmGnomeIndicator {
            uuid,
            visible,
            has_icon,
            position,
            index,
            label,
            image: None,
        });
    }
    with_server(|server| u32::from(server.update_gnome_panel(enabled == 1, indicators)))
        .unwrap_or(0)
}

/// Apply extension-controlled GNOME panel height and edge placement.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_gnome_panel_configure(height: u32, bottom: u32) -> u32 {
    if height == 0 || height > 128 || bottom > 1 {
        return 0;
    }
    with_server(|server| {
        u32::from(server.configure_gnome_panel(u16::try_from(height).unwrap_or(128), bottom == 1))
    })
    .unwrap_or(0)
}

/// Replace the bounded popup menu translated from the extension's original
/// `PopupMenu` actors. Records use a six-byte header (`index`, flags,
/// `label_len: u16`, `icon_kind`, one zero byte) followed by UTF-8 label bytes.
/// Flag bits are separator, reactive, toggle, image, and checked; icon kinds
/// are bounded semantic glyphs, with kind 7 reserved for a decoded file-backed
/// icon.
///
/// # Safety
///
/// A non-empty input pointer must address `input_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_menu_replace(input_ptr: *const u8, input_len: u32) -> u32 {
    if input_len != 0 && input_ptr.is_null() {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_len > 64 * (6 + 192) {
        return 0;
    }
    let input = if input_len == 0 {
        &[]
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    let mut items = Vec::new();
    let mut cursor = 0usize;
    while cursor < input.len() {
        if input.len().saturating_sub(cursor) < 6 || items.len() >= 64 {
            return 0;
        }
        let index = input[cursor];
        let flags = input[cursor + 1];
        let label_len = usize::from(u16::from_le_bytes([input[cursor + 2], input[cursor + 3]]));
        let icon_kind = input[cursor + 4];
        if flags & !0x1f != 0 || icon_kind > 16 || input[cursor + 5] != 0 {
            return 0;
        }
        cursor += 6;
        let Some(end) = cursor.checked_add(label_len) else {
            return 0;
        };
        if label_len > 192 || end > input.len() {
            return 0;
        }
        items.push(WmGnomeMenuItem {
            index,
            separator: flags & 0x01 != 0,
            reactive: flags & 0x02 != 0,
            toggle: flags & 0x04 != 0,
            icon_kind,
            image: flags & 0x08 != 0,
            checked: flags & 0x10 != 0,
            label: input[cursor..end].to_vec(),
        });
        cursor = end;
    }
    with_server(|server| u32::from(server.update_gnome_menu(items))).unwrap_or(0)
}

/// Remember which extension owns the open popup menu so it anchors to the
/// correct indicator.
///
/// # Safety
///
/// `input_ptr` must address `input_len` readable bytes when nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_menu_indicator_update(
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_len == 0 || input_len > 128 || input_ptr.is_null() {
        return 0;
    }
    // SAFETY: the caller contract above supplies readable uuid bytes.
    let uuid = unsafe { slice::from_raw_parts(input_ptr, input_len) }.to_vec();
    with_server(|server| u32::from(server.set_gnome_menu_indicator(uuid))).unwrap_or(0)
}

/// Render a bounded centered modal dialog for an extension's first-run or
/// blocking prompt. `buttons` is `u32 count`, then per button `u32 len` +
/// UTF-8 label bytes.
///
/// # Safety
///
/// `uuid_ptr`/`title_ptr`/`buttons_ptr` must address their declared lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_dialog_replace(
    uuid_ptr: *const u8,
    uuid_len: u32,
    title_ptr: *const u8,
    title_len: u32,
    body_ptr: *const u8,
    body_len: u32,
    buttons_ptr: *const u8,
    buttons_len: u32,
) -> u32 {
    let (Ok(uuid_len), Ok(title_len), Ok(body_len), Ok(buttons_len)) = (
        usize::try_from(uuid_len),
        usize::try_from(title_len),
        usize::try_from(body_len),
        usize::try_from(buttons_len),
    ) else {
        return 0;
    };
    if uuid_len == 0
        || uuid_len > 128
        || title_len > 256
        || body_len > 8192
        || buttons_len > 1024
        || uuid_ptr.is_null()
        || title_ptr.is_null()
        || (body_len != 0 && body_ptr.is_null())
        || (buttons_len != 0 && buttons_ptr.is_null())
    {
        return 0;
    }
    // SAFETY: the caller contract above supplies readable bytes.
    let uuid = unsafe { slice::from_raw_parts(uuid_ptr, uuid_len) }.to_vec();
    let title = unsafe { slice::from_raw_parts(title_ptr, title_len) }.to_vec();
    let decode_list = |bytes: &[u8], maximum_count: usize, maximum_length: usize| {
        if bytes.len() < 4 {
            return None;
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
        if count > maximum_count {
            return None;
        }
        let mut output = Vec::with_capacity(count);
        let mut cursor = 4usize;
        for _ in 0..count {
            if cursor + 4 > bytes.len() {
                return None;
            }
            let len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?) as usize;
            cursor += 4;
            if len == 0 || len > maximum_length || cursor + len > bytes.len() {
                return None;
            }
            output.push(bytes[cursor..cursor + len].to_vec());
            cursor += len;
        }
        (cursor == bytes.len()).then_some(output)
    };
    let body_bytes = unsafe { slice::from_raw_parts(body_ptr, body_len) };
    let button_bytes = unsafe { slice::from_raw_parts(buttons_ptr, buttons_len) };
    let Some(body) = decode_list(body_bytes, 24, 256) else {
        return 0;
    };
    let Some(buttons) = decode_list(button_bytes, 16, 64) else {
        return 0;
    };
    with_server(|server| u32::from(server.update_gnome_dialog(uuid, title, body, buttons)))
        .unwrap_or(0)
}

/// Hide the extension modal dialog overlay.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_gnome_dialog_clear() -> u32 {
    with_server(|server| u32::from(server.clear_gnome_dialog())).unwrap_or(0)
}

/// Present the current extension dialog content as a stage-sized St overlay.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_gnome_dialog_fullscreen(enabled: u32) -> u32 {
    if enabled > 1 {
        return 0;
    }
    with_server(|server| u32::from(server.set_gnome_dialog_fullscreen(enabled == 1))).unwrap_or(0)
}

/// Replace bounded geometry translated from a live GNOME St overlay tree.
/// Each 12-byte record is x/y/width/height as little-endian u16 followed by
/// an RGB u32; the stream begins with a u32 record count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_overlay_rects_update(
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_ptr.is_null() || input_len < 4 || input_len > 4 + 512 * 12 {
        return 0;
    }
    // SAFETY: the caller provides a readable Wasm-memory range.
    let bytes = unsafe { slice::from_raw_parts(input_ptr, input_len) };
    let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    if count > 512 || input_len != 4 + count * 12 {
        return 0;
    }
    let mut rectangles = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 4 + index * 12;
        let read = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
        let x = read(offset);
        let y = read(offset + 2);
        let width = read(offset + 4);
        let height = read(offset + 6);
        let color = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
        if width == 0 || height == 0 {
            return 0;
        }
        rectangles.push(WmGnomeOverlayRect {
            x,
            y,
            width,
            height,
            color,
        });
    }
    with_server(|server| u32::from(server.update_gnome_overlay_rects(rectangles))).unwrap_or(0)
}

/// Replace the bounded translucent actor/text layer rendered behind desktop
/// windows for GNOME extensions using `Main.layoutManager._backgroundGroup`.
///
/// The stream begins with little-endian u16 rectangle/text counts. Rectangles
/// use the existing 12-byte overlay geometry record. Each text record has a
/// 16-byte header (x/y/width/height, font/weight/alignment/length, ARGB) and a
/// four-byte-aligned UTF-8 payload.
///
/// # Safety
///
/// `input_ptr` must address exactly `input_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_background_update(
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_ptr.is_null() || !(4..=2 * 1024 * 1024).contains(&input_len) {
        return 0;
    }
    // SAFETY: upheld by the exported function's caller contract.
    let bytes = unsafe { slice::from_raw_parts(input_ptr, input_len) };
    let rectangle_count = usize::from(u16::from_le_bytes(bytes[0..2].try_into().unwrap()));
    let text_count = usize::from(u16::from_le_bytes(bytes[2..4].try_into().unwrap()));
    if rectangle_count > 512 || text_count > 4096 {
        return 0;
    }
    let Some(mut cursor) = 4usize.checked_add(rectangle_count.saturating_mul(12)) else {
        return 0;
    };
    if cursor > bytes.len() {
        return 0;
    }
    let mut rectangles = Vec::with_capacity(rectangle_count);
    for index in 0..rectangle_count {
        let offset = 4 + index * 12;
        let read = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
        let width = read(offset + 4);
        let height = read(offset + 6);
        if width == 0 || height == 0 {
            return 0;
        }
        rectangles.push(WmGnomeOverlayRect {
            x: read(offset),
            y: read(offset + 2),
            width,
            height,
            color: u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
        });
    }
    let mut texts = Vec::with_capacity(text_count);
    for _ in 0..text_count {
        if cursor + 16 > bytes.len() {
            return 0;
        }
        let read = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
        let text_len = usize::from(bytes[cursor + 11]);
        let Some(text_end) = cursor.checked_add(16 + text_len) else {
            return 0;
        };
        if text_len == 0
            || text_end > bytes.len()
            || core::str::from_utf8(&bytes[cursor + 16..text_end]).is_err()
        {
            return 0;
        }
        let width = read(cursor + 4);
        let height = read(cursor + 6);
        if width == 0 || height == 0 || bytes[cursor + 8] == 0 || bytes[cursor + 10] > 2 {
            return 0;
        }
        texts.push(WmGnomeBackgroundText {
            x: read(cursor),
            y: read(cursor + 2),
            width,
            height,
            font_px: bytes[cursor + 8],
            weight: bytes[cursor + 9],
            align: bytes[cursor + 10],
            foreground: u32::from_le_bytes(bytes[cursor + 12..cursor + 16].try_into().unwrap()),
            text: bytes[cursor + 16..text_end].to_vec(),
        });
        cursor = (text_end + 3) & !3;
        if cursor > bytes.len() {
            return 0;
        }
    }
    if cursor != bytes.len() {
        return 0;
    }
    with_server(|server| u32::from(server.update_gnome_background(rectangles, texts))).unwrap_or(0)
}

/// Upload decoded ARGB pixels for the extension popup's bounded image row.
///
/// # Safety
///
/// `pixels_ptr` must address exactly `pixel_len` readable `u32` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_menu_image_update(
    width: u32,
    height: u32,
    pixels_ptr: *const u32,
    pixel_len: u32,
) -> u32 {
    let (Ok(width), Ok(height), Ok(pixel_len)) = (
        u16::try_from(width),
        u16::try_from(height),
        usize::try_from(pixel_len),
    ) else {
        return 0;
    };
    let expected = usize::from(width).saturating_mul(usize::from(height));
    if pixels_ptr.is_null() || expected == 0 || expected != pixel_len || expected > 512 * 512 {
        return 0;
    }
    // SAFETY: upheld by the exported function's caller contract.
    let pixels = unsafe { slice::from_raw_parts(pixels_ptr, pixel_len) }.to_vec();
    with_server(|server| u32::from(server.update_gnome_menu_image(width, height, pixels)))
        .unwrap_or(0)
}

/// Upload one decoded file-backed popup icon.
///
/// # Safety
///
/// `pixels_ptr` must address exactly `pixel_len` readable `u32` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_menu_icon_update(
    index: u32,
    width: u32,
    height: u32,
    pixels_ptr: *const u32,
    pixel_len: u32,
) -> u32 {
    let (Ok(index), Ok(width), Ok(height), Ok(pixel_len)) = (
        u8::try_from(index),
        u16::try_from(width),
        u16::try_from(height),
        usize::try_from(pixel_len),
    ) else {
        return 0;
    };
    let expected = usize::from(width).saturating_mul(usize::from(height));
    if pixels_ptr.is_null() || expected == 0 || expected != pixel_len || expected > 128 * 128 {
        return 0;
    }
    // SAFETY: upheld by the exported function's caller contract.
    let pixels = unsafe { slice::from_raw_parts(pixels_ptr, pixel_len) }.to_vec();
    with_server(|server| u32::from(server.update_gnome_menu_icon(index, width, height, pixels)))
        .unwrap_or(0)
}

/// Upload one decoded file-backed top-panel indicator icon.
///
/// # Safety
///
/// `pixels_ptr` must address exactly `pixel_len` readable `u32` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_gnome_indicator_image_update(
    uuid_ptr: *const u8,
    uuid_len: u32,
    width: u32,
    height: u32,
    pixels_ptr: *const u32,
    pixel_len: u32,
) -> u32 {
    let (Ok(uuid_len), Ok(width), Ok(height), Ok(pixel_len)) = (
        usize::try_from(uuid_len),
        u16::try_from(width),
        u16::try_from(height),
        usize::try_from(pixel_len),
    ) else {
        return 0;
    };
    if uuid_len == 0 || uuid_len > 128 || (uuid_len != 0 && uuid_ptr.is_null()) {
        return 0;
    }
    let expected = usize::from(width).saturating_mul(usize::from(height));
    if pixels_ptr.is_null() || expected == 0 || expected != pixel_len || expected > 128 * 128 {
        return 0;
    }
    // SAFETY: the caller contract above supplies readable uuid bytes.
    let uuid = unsafe { slice::from_raw_parts(uuid_ptr, uuid_len) }.to_vec();
    // SAFETY: upheld by the exported function's caller contract.
    let pixels = unsafe { slice::from_raw_parts(pixels_ptr, pixel_len) }.to_vec();
    with_server(|server| {
        u32::from(server.update_gnome_indicator_image(uuid, width, height, pixels))
    })
    .unwrap_or(0)
}

/// Toggle the taskbar's cross-toolkit hover preview popover.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_task_previews_set(enabled: u32) -> u32 {
    if enabled > 1 {
        return 0;
    }
    with_server(|server| u32::from(server.set_task_previews_enabled(enabled == 1))).unwrap_or(0)
}

/// Set the RiceWM launcher appearance and its UTF-8 custom label.
///
/// # Safety
///
/// When `text_len` is nonzero, `text_ptr` must address exactly that many
/// readable bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_launcher_appearance_set(
    appearance: u32,
    text_ptr: *const u8,
    text_len: u32,
) -> u32 {
    let (Ok(appearance), Ok(text_len)) = (u8::try_from(appearance), usize::try_from(text_len))
    else {
        return 0;
    };
    if text_len == 0 || text_len > 96 || text_ptr.is_null() {
        return 0;
    }
    // SAFETY: upheld by the exported function's caller contract.
    let text = unsafe { slice::from_raw_parts(text_ptr, text_len) }.to_vec();
    with_server(|server| u32::from(server.set_launcher_appearance(appearance, text))).unwrap_or(0)
}

/// Atomically replace the ordered tray plugin instances.
///
/// Each twelve-byte record contains `plugin: u8`, `icon: u8`, two reserved
/// zero bytes, launcher action `u32`, and icon color `u32`. Plugins are
/// clock=1, workspaces=2, sound=3, custom launcher=4, performance graph=5,
/// and connectivity strategy=6.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must address its declared number
/// of readable bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_tray_replace(input_ptr: *const u8, input_len: u32) -> u32 {
    if input_len != 0 && input_ptr.is_null() {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_len % 12 != 0 || input_len / 12 > MAX_TRAY_ITEMS {
        return 0;
    }
    let input = if input_len == 0 {
        &[][..]
    } else {
        // SAFETY: the caller contract above supplies readable Wasm memory.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    let mut items = Vec::with_capacity(input_len / 12);
    for record in input.chunks_exact(12) {
        if record[2..4] != [0, 0] {
            return 0;
        }
        let action = u32::from_le_bytes(record[4..8].try_into().unwrap());
        let color = u32::from_le_bytes(record[8..12].try_into().unwrap());
        let item = match (record[0], action) {
            (1, 0) if record[1] == 0 && color == 0 => TrayItem::Clock,
            (2, 0) if record[1] == 0 && color == 0 => TrayItem::Workspaces,
            (3, 0) if record[1] == 0 && color == 0 => TrayItem::Sound,
            (5, 0) if record[1] == 0 && color == 0 => TrayItem::Performance,
            (6, 0) if record[1] == 0 && color == 0 => TrayItem::Connectivity,
            (4, action) if color <= 0x00ff_ffff => {
                let target = match action {
                    0xffff_ff01 => LauncherTarget::Files,
                    0xffff_ff02 => LauncherTarget::Terminal,
                    0xffff_ff03 => LauncherTarget::ImageViewer,
                    _ => return 0,
                };
                let Some(icon) = TrayIcon::from_index(record[1]) else {
                    return 0;
                };
                TrayItem::Launcher(TrayLauncher {
                    target,
                    icon,
                    color,
                })
            }
            _ => return 0,
        };
        items.push(item);
    }
    with_server(|server| u32::from(server.replace_tray_items(items))).unwrap_or(0)
}

/// Replace the four UTF-8 workspace labels used by the Workspaces tray item.
/// Labels are encoded as a single newline-separated document.
///
/// # Safety
///
/// `input_ptr` must address `input_len` readable bytes in Wasm memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_workspace_names_replace(
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    if input_len == 0 || input_ptr.is_null() {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_len > 256 {
        return 0;
    }
    // SAFETY: the caller contract above supplies readable Wasm memory.
    let input = unsafe { slice::from_raw_parts(input_ptr, input_len) };
    let Ok(text) = core::str::from_utf8(input) else {
        return 0;
    };
    let names = text
        .split('\n')
        .map(|name| name.as_bytes().to_vec())
        .collect();
    with_server(|server| u32::from(server.replace_workspace_names(names))).unwrap_or(0)
}

/// Replace the ordered quick shortcuts beside the RiceWM button.
///
/// Each record is 72 bytes: launcher action `u32`, icon `u8`, UTF-8 title
/// length `u8`, two reserved zero bytes, and a zero-padded 64-byte title.
/// The list is independently showable/hideable.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must address its declared number
/// of readable bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_quick_launch_replace(
    visible: u32,
    input_ptr: *const u8,
    input_len: u32,
) -> u32 {
    if visible > 1 || (input_len != 0 && input_ptr.is_null()) {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    const RECORD_BYTES: usize = 72;
    const TITLE_BYTES: usize = 64;
    if input_len % RECORD_BYTES != 0 || input_len / RECORD_BYTES > 8 {
        return 0;
    }
    let input = if input_len == 0 {
        &[][..]
    } else {
        // SAFETY: the caller contract above supplies readable Wasm memory.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    let mut launchers = Vec::with_capacity(input_len / RECORD_BYTES);
    for record in input.chunks_exact(RECORD_BYTES) {
        let title_len = usize::from(record[5]);
        if record[6..8] != [0, 0]
            || title_len == 0
            || title_len > TITLE_BYTES
            || record[8 + title_len..RECORD_BYTES]
                .iter()
                .any(|byte| *byte != 0)
        {
            return 0;
        }
        launchers.push(WmQuickLauncher {
            action: u32::from_le_bytes(record[..4].try_into().unwrap()),
            icon: record[4],
            title: record[8..8 + title_len].to_vec(),
        });
    }
    with_server(|server| u32::from(server.replace_quick_launchers(visible == 1, launchers)))
        .unwrap_or(0)
}

/// Append a live CPU/network sample to the native taskbar graph.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_performance_sample(cpu: u32, network: u32) -> u32 {
    let (Ok(cpu), Ok(network)) = (u8::try_from(cpu), u8::try_from(network)) else {
        return 0;
    };
    with_server(|server| u32::from(server.set_performance_sample(cpu, network))).unwrap_or(0)
}

/// Feed pointer proximity to the taskbar auto-hide sensor without delivering
/// a duplicate motion event to an X11 client beneath a native desktop layer.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_taskbar_pointer(root_x: i32, root_y: i32) -> u32 {
    let (Ok(root_x), Ok(root_y)) = (i16::try_from(root_x), i16::try_from(root_y)) else {
        return 0;
    };
    with_server(|server| {
        server.update_taskbar_pointer(root_x, root_y);
        1
    })
    .unwrap_or(0)
}

/// Return the authoritative RiceWM workspace used by X11 and native tasks.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_current_workspace() -> u32 {
    with_server(|server| server.current_workspace()).unwrap_or(RESULT_ERROR)
}

/// Switch the authoritative RiceWM workspace through the same compositor
/// transaction used by taskbar clicks and global workspace shortcuts.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_workspace_switch(workspace: u32) -> u32 {
    with_server(|server| {
        let previous = server.current_workspace();
        server.switch_workspace(workspace);
        u32::from(server.current_workspace() != previous)
    })
    .unwrap_or(0)
}

/// Atomically replace browser-native taskbar entries.
///
/// Each record has an eight-byte header (`id: u16`, `workspace: u8`,
/// `flags: u8`, `icon: u8`, `title_len: u8`, two reserved zero bytes),
/// followed by `title_len` title bytes. Flag bit 0 is minimized, bit 1 is
/// active, and bit 2 is maximized.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must address its declared number
/// of readable bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_native_tasks_replace(input_ptr: *const u8, input_len: u32) -> u32 {
    if input_len != 0 && input_ptr.is_null() {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    let input = if input_len == 0 {
        &[]
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    let mut tasks = Vec::new();
    let mut cursor = 0usize;
    while cursor < input.len() {
        if input.len().saturating_sub(cursor) < 8 {
            return 0;
        }
        let id = u16::from_le_bytes([input[cursor], input[cursor + 1]]);
        let workspace = u32::from(input[cursor + 2]);
        let flags = input[cursor + 3];
        let icon = input[cursor + 4];
        let title_len = usize::from(input[cursor + 5]);
        if flags & !0x07 != 0 || input[cursor + 6] != 0 || input[cursor + 7] != 0 {
            return 0;
        }
        cursor += 8;
        let Some(title_end) = cursor.checked_add(title_len) else {
            return 0;
        };
        if title_end > input.len() {
            return 0;
        }
        tasks.push(WmNativeTask {
            id,
            workspace,
            icon,
            minimized: flags & 0x01 != 0,
            maximized: flags & 0x04 != 0,
            active: flags & 0x02 != 0,
            title: input[cursor..title_end].to_vec(),
        });
        cursor = title_end;
    }
    with_server(|server| u32::from(server.replace_native_tasks(tasks))).unwrap_or(0)
}

/// Open the compositor-owned window menu for a browser-native task at a root
/// coordinate supplied by its retained title-bar icon.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_native_task_menu_show(id: u32, root_x: i32, root_y: i32) -> u32 {
    let Ok(id) = u16::try_from(id) else {
        return 0;
    };
    with_server(|server| u32::from(server.show_native_task_menu(id, root_x, root_y))).unwrap_or(0)
}

/// Select an entry in the current bounded RiceWM theme catalog.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_theme_select(theme: u32) -> u32 {
    let Ok(theme) = u8::try_from(theme) else {
        return 0;
    };
    with_server(|server| u32::from(server.select_wm_theme(theme))).unwrap_or(0)
}

/// Show a native `RiceWM` notification above the managed desktop.
///
/// The title and message use printable ASCII so the fixed bitmap font has a
/// deterministic, allocation-bounded representation.
///
/// # Safety
///
/// Each non-empty input pointer must address its declared number of readable
/// bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_notification_show(
    kind: u32,
    title_ptr: *const u8,
    title_len: u32,
    message_ptr: *const u8,
    message_len: u32,
) -> u32 {
    let (Ok(kind), Ok(title_len), Ok(message_len)) = (
        u8::try_from(kind),
        usize::try_from(title_len),
        usize::try_from(message_len),
    ) else {
        return 0;
    };
    if title_len != 0 && title_ptr.is_null() || message_len != 0 && message_ptr.is_null() {
        return 0;
    }
    let title = if title_len == 0 {
        Vec::new()
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(title_ptr, title_len) }.to_vec()
    };
    let message = if message_len == 0 {
        Vec::new()
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(message_ptr, message_len) }.to_vec()
    };
    with_server(|server| u32::from(server.show_wm_notification(kind, title, message))).unwrap_or(0)
}

/// Hide the current native `RiceWM` notification.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_notification_clear() -> u32 {
    with_server(|server| u32::from(server.clear_wm_notification())).unwrap_or(0)
}

/// Dismiss the RiceWM shell menu without delivering a pointer event to an
/// underlying X11 client. Browser-native windows use this before handling the
/// click that takes focus away from the menu.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_menu_dismiss() -> u32 {
    with_server(|server| u32::from(server.dismiss_wm_menu())).unwrap_or(0)
}

/// Re-announce mapped client surfaces after the browser transport has made a
/// `MapWindow` transaction visible to the guest event loop.
#[unsafe(no_mangle)]
pub extern "C" fn rice_refresh_viewable_windows() -> u32 {
    with_server(|server| {
        u32::try_from(server.refresh_viewable_client_windows()).unwrap_or(u32::MAX)
    })
    .unwrap_or(RESULT_ERROR)
}

/// Reserve the next ordinary managed X11 top-level for invisible prewarming.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_park_next_managed_window() -> u32 {
    with_server(|server| u32::from(server.request_park_next_managed_window())).unwrap_or(0)
}

/// Cancel a reservation that has not yet acquired a managed window.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_park_cancel() -> u32 {
    with_server(|server| u32::from(server.cancel_park_next_managed_window())).unwrap_or(0)
}

/// Set whether a managed X11 client accepts compositor-driven resize.
/// Toolkits and compatibility hosts derive this from their native window
/// style/capability metadata rather than application identity.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_managed_window_set_resizable(window: u32, resizable: u32) -> u32 {
    if window == 0 || resizable > 1 {
        return 0;
    }
    with_server(|server| u32::from(server.set_managed_window_resizable(window, resizable == 1)))
        .unwrap_or(0)
}

/// Return the parked managed X11 client id, or zero while none is ready.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_parked_window() -> u32 {
    with_server(|server| server.parked_managed_window().unwrap_or(0)).unwrap_or(0)
}

/// Return the display client handle that owns the parked managed window.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_parked_window_owner() -> u32 {
    with_server(|server| server.parked_managed_window_owner().unwrap_or(0)).unwrap_or(0)
}

/// Reveal a parked application while retaining its already-uploaded surfaces.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_parked_window_activate(window: u32) -> u32 {
    if window == 0 {
        return 0;
    }
    with_server(|server| u32::from(server.activate_parked_managed_window(window))).unwrap_or(0)
}

/// Apply workspace visibility without re-emitting a shell action.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_desktop_hidden(hidden: u32) -> u32 {
    if hidden > 1 { return 0; }
    with_server(|server| { server.set_desktop_hidden(hidden == 1); 1 }).unwrap_or(0)
}

/// Restore minimized X11 windows on the current workspace.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_restore_workspace_windows() -> u32 {
    with_server(|server| { server.restore_workspace_windows(); 1 }).unwrap_or(0)
}

/// Copy the window search catalog, or return its required length with capacity 0.
///
/// # Safety
/// A nonzero capacity requires a writable region of that size in linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_search_windows(output_ptr: *mut u8, capacity: u32) -> u32 {
    with_server(|server| {
        let bytes = server.search_window_snapshot();
        let Ok(length) = u32::try_from(bytes.len()) else { return RESULT_ERROR; };
        if capacity == 0 { return length; }
        if output_ptr.is_null() || capacity < length { return RESULT_ERROR; }
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output_ptr, bytes.len()); }
        length
    }).unwrap_or(RESULT_ERROR)
}

/// Copy bounded spatial ownership metadata; capacity zero returns required size.
/// # Safety
/// A nonzero capacity requires a writable output region of that size.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_spatial_windows(output_ptr: *mut u8, capacity: u32) -> u32 {
    with_server(|server| {
        let Some(bytes) = server.spatial_window_snapshot() else { return RESULT_ERROR; };
        let length = bytes.len() as u32;
        if capacity == 0 { return length; }
        if output_ptr.is_null() || capacity < length { return RESULT_ERROR; }
        // SAFETY: upheld by the exported caller contract.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output_ptr, bytes.len()); }
        length
    }).unwrap_or(RESULT_ERROR)
}

/// Return a current-workspace X11 task frame, including minimized windows.
/// Zero marks the end of the list; owned and parked windows are excluded.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_switcher_window(index: u32) -> u32 {
    with_server(|server| server.switcher_window(index as usize).unwrap_or(0)).unwrap_or(0)
}

/// Resolve a visible overview frame to its X11 client without activation.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_overview_client(window: u32) -> u32 {
    with_server(|server| server.overview_client(window).unwrap_or(0)).unwrap_or(0)
}

/// Raise a retained X11 frame selected by the browser Activities overview.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_overview_activate(window: u32) -> u32 {
    if window == 0 {
        return 0;
    }
    with_server(|server| u32::from(server.activate_overview_window(window))).unwrap_or(0)
}

/// Move a retained Activities frame to a workspace and desktop position.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_overview_move(window: u32, workspace: u32, x: i32, y: i32) -> u32 {
    with_server(|server| u32::from(server.move_overview_window(window, workspace, x, y))).unwrap_or(0)
}

/// Request a standards-compliant close for an X11 frame selected in Activities.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_overview_close(window: u32) -> u32 {
    if window == 0 {
        return 0;
    }
    with_server(|server| u32::from(server.close_overview_window(window))).unwrap_or(0)
}

/// Close and forget a parked application without publishing its scene.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_parked_window_discard(window: u32) -> u32 {
    if window == 0 {
        return 0;
    }
    with_server(|server| u32::from(server.discard_parked_managed_window(window))).unwrap_or(0)
}

/// Atomically replace the native application menu.
///
/// Each record is an eight-byte header (`action: u32`, `icon: u8`,
/// `label_len: u8`, `category: u8`, one reserved byte) followed by
/// `label_len` label bytes.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must point to a readable region of
/// at least `input_len` bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_menu_replace(input_ptr: *const u8, input_len: u32) -> u32 {
    if input_len != 0 && input_ptr.is_null() {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    let input = if input_len == 0 {
        &[]
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    let mut entries = Vec::new();
    let mut cursor = 0usize;
    while cursor < input.len() {
        if input.len().saturating_sub(cursor) < 8 {
            return 0;
        }
        let action = u32::from_le_bytes([
            input[cursor],
            input[cursor + 1],
            input[cursor + 2],
            input[cursor + 3],
        ]);
        let icon = input[cursor + 4];
        let label_len = usize::from(input[cursor + 5]);
        let category = input[cursor + 6];
        if input[cursor + 7] != 0 {
            return 0;
        }
        cursor += 8;
        let Some(label_end) = cursor.checked_add(label_len) else {
            return 0;
        };
        if label_end > input.len() {
            return 0;
        }
        entries.push((action, icon, category, input[cursor..label_end].to_vec()));
        cursor = label_end;
    }
    with_server(|server| u32::from(server.replace_wm_menu_entries(entries))).unwrap_or(0)
}

/// Atomically replace the native application menu from filesystem folders.
///
/// Each record is a ten-byte header (`action: u32`, `icon: u8`,
/// `label_len: u8`, `path_len: u16`, `flags: u16`), followed by the label and
/// a NUL-separated relative folder path. Flags identify uninstallable,
/// provisionable, and actively hydrating applications. The v1 export remains
/// for older web clients and prebuilt runtime assets.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must point to a readable region of
/// at least `input_len` bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_menu_replace_v2(input_ptr: *const u8, input_len: u32) -> u32 {
    if input_len != 0 && input_ptr.is_null() {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    let input = if input_len == 0 {
        &[]
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    let mut entries = Vec::new();
    let mut cursor = 0usize;
    while cursor < input.len() {
        if input.len().saturating_sub(cursor) < 10 {
            return 0;
        }
        let action = u32::from_le_bytes(input[cursor..cursor + 4].try_into().unwrap_or_default());
        let icon = input[cursor + 4];
        let label_len = usize::from(input[cursor + 5]);
        let path_len = usize::from(u16::from_le_bytes([input[cursor + 6], input[cursor + 7]]));
        let flags = u16::from_le_bytes([input[cursor + 8], input[cursor + 9]]);
        if flags & !7 != 0 || path_len > 384 {
            return 0;
        }
        cursor += 10;
        let Some(label_end) = cursor.checked_add(label_len) else {
            return 0;
        };
        let Some(path_end) = label_end.checked_add(path_len) else {
            return 0;
        };
        if path_end > input.len() {
            return 0;
        }
        let path = if path_len == 0 {
            Vec::new()
        } else {
            input[label_end..path_end]
                .split(|byte| *byte == 0)
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>()
        };
        entries.push((
            action,
            icon,
            flags & 1 != 0,
            flags & 2 != 0,
            flags & 4 != 0,
            path,
            input[cursor..label_end].to_vec(),
        ));
        cursor = path_end;
    }
    with_server(|server| u32::from(server.replace_wm_structured_menu_entries(entries))).unwrap_or(0)
}

/// Atomically replace the bounded data-only `RiceWM` theme catalog.
///
/// Each record starts with four bytes (`name_len | material << 7`,
/// `chrome_profile << 4 | taskbar_count`, menu count 6, frame count 8),
/// followed by 21 little-endian RGB `u32` values and `name_len` bounded ASCII
/// display bytes. Material bit 7 requests compositor-backed blur for
/// translucent theme surfaces; profile codes 0 through 5 select bounded chrome.
///
/// # Safety
///
/// When `input_len` is nonzero, `input_ptr` must point to a readable region of
/// at least `input_len` bytes in this Wasm instance's linear memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_themes_replace(input_ptr: *const u8, input_len: u32) -> u32 {
    if input_len != 0 && input_ptr.is_null() {
        return 0;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    let input = if input_len == 0 {
        &[]
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts(input_ptr, input_len) }
    };
    let mut themes = Vec::new();
    let mut cursor = 0usize;
    while cursor < input.len() {
        if input.len().saturating_sub(cursor) < 4 {
            return 0;
        }
        let backdrop_material = input[cursor] & 0x80 != 0;
        let name_len = usize::from(input[cursor] & 0x7f);
        let menu_backdrop_material = input[cursor + 1] & 0x08 != 0;
        if input[cursor + 1] & 0x07 != 7 || input[cursor + 2..cursor + 4] != [6, 8] {
            return 0;
        }
        let chrome_profile = match input[cursor + 1] >> 4 {
            0 => rice_x11_core::WmChromeProfile::Standard,
            1 => rice_x11_core::WmChromeProfile::Familiar,
            2 => rice_x11_core::WmChromeProfile::YellowJacket,
            3 => rice_x11_core::WmChromeProfile::CommandDeck,
            4 => rice_x11_core::WmChromeProfile::Orchard,
            5 => rice_x11_core::WmChromeProfile::NeonGrid,
            6 => rice_x11_core::WmChromeProfile::SplitControls,
            7 => rice_x11_core::WmChromeProfile::Redmond98,
            _ => return 0,
        };
        cursor += 4;
        let color_bytes = (7 + 6 + 8) * 4;
        if input.len().saturating_sub(cursor) < color_bytes {
            return 0;
        }
        let mut colors = [0u32; 21];
        for color in &mut colors {
            *color = u32::from_le_bytes(input[cursor..cursor + 4].try_into().unwrap_or_default());
            cursor += 4;
        }
        let Some(name_end) = cursor.checked_add(name_len) else {
            return 0;
        };
        if name_end > input.len() {
            return 0;
        }
        let mut taskbar = [0u32; 7];
        let mut menu = [0u32; 6];
        let mut frame = [0u32; 8];
        taskbar.copy_from_slice(&colors[..7]);
        menu.copy_from_slice(&colors[7..13]);
        frame.copy_from_slice(&colors[13..]);
        themes.push((
            input[cursor..name_end].to_vec(),
            backdrop_material,
            menu_backdrop_material,
            chrome_profile,
            taskbar,
            menu,
            frame,
        ));
        cursor = name_end;
    }
    with_server(|server| u32::from(server.replace_wm_themes(themes))).unwrap_or(0)
}

/// Return the number of pending native application-menu actions.
/// Drain a bounded ICCCM WM_COMMAND launch request.
///
/// # Safety
/// `output_ptr` must address `capacity` writable bytes in Wasm memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_relaunch_command_drain(output_ptr:*mut u8,capacity:u32)->u32 {
    if output_ptr.is_null() || capacity>4096 {return 0;}
    let output=unsafe{slice::from_raw_parts_mut(output_ptr,capacity as usize)};
    with_server(|server|server.drain_wm_relaunch_command(output) as u32).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_action_pending() -> u32 {
    with_server(|server| u32::try_from(server.pending_wm_actions()).unwrap_or(RESULT_ERROR))
        .unwrap_or(RESULT_ERROR)
}

/// Drain one native application-menu action, or zero when the queue is empty.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_action_drain() -> u32 {
    with_server(|server| server.drain_wm_action().unwrap_or(0)).unwrap_or(0)
}

/// Return the number of renderer command bytes waiting to be drained.
#[unsafe(no_mangle)]
pub extern "C" fn rice_scene_pending() -> u32 {
    with_server(|server| u32::try_from(server.pending_scene()).unwrap_or(RESULT_ERROR))
        .unwrap_or(RESULT_ERROR)
}

/// Drain renderer commands into browser-visible linear memory.
///
/// The returned byte count is always a multiple of the fixed scene command
/// size. Returns `u32::MAX` when `RiceWM` is not initialized.
///
/// # Safety
///
/// When `output_capacity` is nonzero, `output_ptr` must point to a writable
/// region of at least `output_capacity` bytes in this Wasm instance's linear
/// memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_scene_drain(output_ptr: *mut u8, output_capacity: u32) -> u32 {
    if output_capacity != 0 && output_ptr.is_null() {
        return RESULT_ERROR;
    }
    let Ok(output_capacity) = usize::try_from(output_capacity) else {
        return RESULT_ERROR;
    };
    let output = if output_capacity == 0 {
        &mut []
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts_mut(output_ptr, output_capacity) }
    };
    with_server(|server| u32::try_from(server.drain_scene(output)).unwrap_or(RESULT_ERROR))
        .unwrap_or(RESULT_ERROR)
}

/// Return the bytes required for the current semantic native-chrome text
/// snapshot consumed by the browser WOFF2 renderer.
#[unsafe(no_mangle)]
pub extern "C" fn rice_shell_text_snapshot_len() -> u32 {
    with_server(|server| u32::try_from(server.shell_text_snapshot_len()).unwrap_or(RESULT_ERROR))
        .unwrap_or(RESULT_ERROR)
}

/// Serialize the complete native-chrome text snapshot.
///
/// # Safety
///
/// When `output_capacity` is nonzero, `output_ptr` must point to a writable
/// region of at least `output_capacity` bytes in this Wasm instance.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_shell_text_snapshot(
    output_ptr: *mut u8,
    output_capacity: u32,
) -> u32 {
    if output_capacity != 0 && output_ptr.is_null() {
        return RESULT_ERROR;
    }
    let Ok(output_capacity) = usize::try_from(output_capacity) else {
        return RESULT_ERROR;
    };
    let output = if output_capacity == 0 {
        &mut []
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts_mut(output_ptr, output_capacity) }
    };
    with_server(|server| {
        server
            .write_shell_text_snapshot(output)
            .and_then(|written| u32::try_from(written).ok())
            .unwrap_or(RESULT_ERROR)
    })
    .unwrap_or(RESULT_ERROR)
}

/// Return packed `height << 16 | width` for a drawable surface.
#[unsafe(no_mangle)]
pub extern "C" fn rice_surface_geometry(drawable: u32) -> u32 {
    with_server(|server| {
        server
            .surface_geometry(drawable)
            .map_or(0, |(width, height)| {
                (u32::from(height) << 16) | u32::from(width)
            })
    })
    .unwrap_or(0)
}

/// Read one tightly packed RGBA damage rectangle from a drawable.
///
/// # Safety
///
/// `output_ptr` must address a writable region of `output_capacity` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_surface_read(
    drawable: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    output_ptr: *mut u8,
    output_capacity: u32,
) -> u32 {
    let (Ok(x), Ok(y), Ok(width), Ok(height), Ok(output_capacity)) = (
        u16::try_from(x),
        u16::try_from(y),
        u16::try_from(width),
        u16::try_from(height),
        usize::try_from(output_capacity),
    ) else {
        return RESULT_ERROR;
    };
    if output_capacity != 0 && output_ptr.is_null() {
        return RESULT_ERROR;
    }
    let output = if output_capacity == 0 {
        &mut []
    } else {
        // SAFETY: upheld by the exported function's caller contract.
        unsafe { slice::from_raw_parts_mut(output_ptr, output_capacity) }
    };
    with_server(|server| {
        server
            .read_surface_rgba(drawable, x, y, width, height, output)
            .and_then(|written| u32::try_from(written).ok())
            .unwrap_or(RESULT_ERROR)
    })
    .unwrap_or(RESULT_ERROR)
}

/// Upload a BGRA8 damage rectangle into an existing drawable.
///
/// # Safety
///
/// `input_ptr` must address `input_length` readable bytes in this Wasm
/// instance. The source can contain row padding described by `stride`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_surface_write_bgra(
    drawable: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    stride: u32,
    input_ptr: *const u8,
    input_length: u32,
) -> u32 {
    const MAX_UPLOAD_BYTES: usize = 4 * 1024 * 1024;
    let (Ok(x), Ok(y), Ok(width), Ok(height), Ok(input_length)) = (
        u16::try_from(x),
        u16::try_from(y),
        u16::try_from(width),
        u16::try_from(height),
        usize::try_from(input_length),
    ) else {
        return 0;
    };
    if input_length == 0 || input_length > MAX_UPLOAD_BYTES || input_ptr.is_null() {
        return 0;
    }
    // SAFETY: upheld by the exported function's caller contract.
    let input = unsafe { slice::from_raw_parts(input_ptr, input_length) };
    with_server(|server| {
        u32::from(server.write_surface_bgra(drawable, x, y, width, height, stride, input))
    })
    .unwrap_or(0)
}

/// Return and clear one unsupported core-opcode counter.
///
/// The low byte is the opcode and the upper 24 bits are its saturated count.
/// Zero means no unsupported requests remain; `u32::MAX` means the display is
/// not initialized.
#[unsafe(no_mangle)]
pub extern "C" fn rice_unsupported_next() -> u32 {
    with_server(|server| {
        server
            .take_unsupported_request()
            .map_or(0, |(opcode, count)| {
                (count.min(0x00ff_ffff) << 8) | u32::from(opcode)
            })
    })
    .unwrap_or(RESULT_ERROR)
}

/// Return and clear one processed-request counter.
///
/// The low byte is the X11 major opcode and the upper 24 bits are its
/// saturated count. Zero means no counters remain.
#[unsafe(no_mangle)]
pub extern "C" fn rice_request_count_next() -> u32 {
    with_server(|server| {
        server.take_request_count().map_or(0, |(opcode, count)| {
            (count.min(0x00ff_ffff) << 8) | u32::from(opcode)
        })
    })
    .unwrap_or(RESULT_ERROR)
}

/// Return and clear one managed client window which became viewable.
///
/// Zero means no presentation boundary remains. Unlike the raw MapWindow
/// opcode counter, this excludes GTK's input-only helper windows and is safe
/// to use as the browser-side first-window readiness signal.
#[unsafe(no_mangle)]
pub extern "C" fn rice_presented_window_next() -> u32 {
    with_server(|server| {
        server
            .take_presented_managed_window()
            .map_or(0, |window| window.0)
    })
    .unwrap_or(RESULT_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn exported_abi_completes_the_native_x11_setup() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_abi_version(), ABI_VERSION);
        assert_eq!(rice_init(800, 600), 1);
        let client = rice_client_open();
        assert_ne!(client, 0);

        let request = [b'l', 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let input = rice_io_reserve(u32::try_from(request.len()).unwrap());
        assert!(!input.is_null());
        // SAFETY: `rice_io_reserve` returned a staging region of this length.
        unsafe {
            input.copy_from_nonoverlapping(request.as_ptr(), request.len());
        }
        // SAFETY: `input` is the readable staging region reserved above.
        let consumed =
            unsafe { rice_client_feed(client, input, u32::try_from(request.len()).unwrap()) };
        assert_eq!(consumed, u32::try_from(request.len()).unwrap());
        assert_eq!(
            rice_client_status(client),
            rice_x11_core::ClientStatus::Ready as u32
        );

        let pending = rice_client_pending(client);
        assert_ne!(pending, 0);
        assert_ne!(pending, RESULT_ERROR);
        let output = rice_io_reserve(pending);
        assert!(!output.is_null());
        // SAFETY: `output` is the writable staging region reserved above.
        let written = unsafe { rice_client_drain(client, output, pending) };
        assert_eq!(written, pending);
        // SAFETY: the drain call initialized `written` bytes in the staging
        // region and no subsequent reserve invalidated it.
        let response =
            unsafe { slice::from_raw_parts(output, usize::try_from(written).unwrap()) }.to_vec();
        assert_eq!(response[0], 1);
        assert_eq!(u16::from_le_bytes(response[2..4].try_into().unwrap()), 11);
        assert_eq!(rice_client_close(client), 1);
    }

    #[test]
    fn exported_abi_rejects_invalid_memory_arguments() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let client = rice_client_open();
        assert_eq!(
            // SAFETY: this deliberately exercises the explicit null check.
            unsafe { rice_client_feed(client, std::ptr::null(), 1) },
            RESULT_ERROR
        );
        assert_eq!(
            // SAFETY: this deliberately exercises the explicit null check.
            unsafe { rice_client_drain(client, std::ptr::null_mut(), 1) },
            RESULT_ERROR
        );
        assert_eq!(
            // SAFETY: this deliberately exercises the explicit null check.
            unsafe { rice_scene_drain(std::ptr::null_mut(), 1) },
            RESULT_ERROR
        );
    }

    #[test]
    fn exported_abi_drains_an_aligned_initial_scene() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let pending = rice_scene_pending();
        assert_eq!(
            pending % u32::try_from(rice_x11_core::SCENE_COMMAND_BYTES).unwrap(),
            0
        );
        assert!(pending >= 64);
        let output = rice_io_reserve(pending);
        assert!(!output.is_null());
        // SAFETY: `output` is the writable staging region reserved above.
        let written = unsafe { rice_scene_drain(output, pending) };
        assert_eq!(written, pending);
        // SAFETY: the drain initialized `written` bytes in the staging region.
        let scene = unsafe { slice::from_raw_parts(output, usize::try_from(written).unwrap()) };
        assert_eq!(scene[0], rice_x11_core::SCENE_RESET);
        assert_eq!(
            scene[rice_x11_core::SCENE_COMMAND_BYTES],
            rice_x11_core::SCENE_UPSERT_WINDOW
        );
    }

    #[test]
    fn exported_abi_resizes_the_live_native_output() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        assert_eq!(rice_resize(1280, 720), 1);
        assert_eq!(rice_surface_geometry(0x100), 1280 | (720 << 16));
        assert_eq!(rice_resize(0, 720), 0);
    }

    #[test]
    fn exported_abi_writes_bounded_native_bgra_damage() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let pixels = [0x10, 0x20, 0x40, 0xff, 0x30, 0x60, 0x90, 0xff];
        assert_eq!(
            // SAFETY: `pixels` remains live for the complete call.
            unsafe {
                rice_surface_write_bgra(
                    0x100,
                    5,
                    6,
                    2,
                    1,
                    8,
                    pixels.as_ptr(),
                    u32::try_from(pixels.len()).unwrap(),
                )
            },
            1
        );
        let mut rgba = [0; 8];
        assert_eq!(
            // SAFETY: `rgba` is writable for its full advertised capacity.
            unsafe {
                rice_surface_read(
                    0x100,
                    5,
                    6,
                    2,
                    1,
                    rgba.as_mut_ptr(),
                    u32::try_from(rgba.len()).unwrap(),
                )
            },
            8
        );
        assert_eq!(rgba, [0x40, 0x20, 0x10, 0xff, 0x90, 0x60, 0x30, 0xff]);
        assert_eq!(
            // SAFETY: the pointer is valid; the short stride is rejected.
            unsafe { rice_surface_write_bgra(0x100, 5, 6, 2, 1, 7, pixels.as_ptr(), 8) },
            0
        );
    }

    #[test]
    fn exported_abi_bounds_managed_window_parking_reservations() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        assert_eq!(rice_wm_parked_window(), 0);
        assert_eq!(rice_wm_park_next_managed_window(), 1);
        assert_eq!(rice_wm_park_next_managed_window(), 0);
        assert_eq!(rice_wm_parked_window_activate(0), 0);
        assert_eq!(rice_wm_park_cancel(), 1);
        assert_eq!(rice_wm_park_cancel(), 0);
    }

    #[test]
    fn exported_abi_senses_auto_hidden_taskbar_without_duplicate_x11_motion() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        assert_eq!(rice_wm_taskbar_configure(1, 0, 1), 1);
        assert_eq!(rice_input_pointer_target(10, 595), 1);
        assert_eq!(rice_wm_taskbar_pointer(400, 595), 1);
        assert_eq!(rice_input_pointer_target(10, 595), 3);
        assert_eq!(rice_wm_taskbar_pointer(400, 500), 1);
        assert_eq!(rice_input_pointer_target(10, 500), 1);
        assert_eq!(rice_wm_taskbar_pointer(400, i32::MAX), 0);
    }

    #[test]
    fn exported_abi_bounds_compact_dock_magnification() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        assert_eq!(rice_wm_dock_magnification_set(1, 65, 45), 1);
        assert_eq!(rice_wm_dock_magnification_set(0, 0, 0), 1);
        assert_eq!(rice_wm_dock_magnification_set(2, 65, 45), 0);
        assert_eq!(rice_wm_dock_magnification_set(1, 101, 45), 0);
        assert_eq!(rice_wm_dock_magnification_set(1, 65, 101), 0);
    }

    #[test]
    fn exported_abi_configures_the_ricewm_menu_layout() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        assert_eq!(rice_wm_menu_layout_configure(1, 0, 0), 1);
        assert_eq!(rice_wm_menu_layout_configure(0, 1, 1), 1);
        assert_eq!(rice_wm_menu_layout_configure(2, 1, 1), 0);
        assert_eq!(rice_wm_menu_icon_size_set(16), 1);
        assert_eq!(rice_wm_menu_icon_size_set(24), 1);
        assert_eq!(rice_wm_menu_icon_size_set(32), 1);
        assert_eq!(rice_wm_menu_icon_size_set(64), 1);
        assert_eq!(rice_wm_menu_icon_size_set(48), 0);
    }

    #[test]
    fn exported_abi_bridges_gnome_overview_elements() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        assert_eq!(rice_wm_gnome_overview_elements_set(0, 0), 1);
        assert_eq!(rice_wm_gnome_overview_elements_set(1, 1), 1);
        assert_eq!(rice_wm_gnome_overview_elements_set(2, 1), 0);
    }

    #[test]
    fn exported_abi_exposes_the_extension_indicator_without_activities() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let label = b"Files";
        // SAFETY: `label` remains readable for the duration of this call.
        assert_eq!(
            unsafe {
                rice_wm_gnome_panel_update(
                    1,
                    1,
                    1,
                    2,
                    label.as_ptr(),
                    u32::try_from(label.len()).unwrap(),
                )
            },
            1,
        );
        let pending = rice_shell_text_snapshot_len();
        let output = rice_io_reserve(pending);
        // SAFETY: the reserved output holds the declared snapshot capacity.
        assert_eq!(
            unsafe { rice_shell_text_snapshot(output, pending) },
            pending
        );
        // SAFETY: the snapshot initialized `pending` bytes in the staging region.
        let snapshot = unsafe { slice::from_raw_parts(output, usize::try_from(pending).unwrap()) };
        assert!(!snapshot.windows(10).any(|bytes| bytes == b"Activities"));
        assert!(snapshot.windows(label.len()).any(|bytes| bytes == label));
        assert_eq!(
            // SAFETY: this deliberately exercises the explicit null check.
            unsafe { rice_wm_gnome_panel_update(1, 1, 0, 2, std::ptr::null(), 1) },
            0,
        );
    }

    #[test]
    fn exported_abi_registers_native_tasks_and_reports_workspace_actions() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        assert_eq!(rice_wm_current_workspace(), 0);
        let mut record = vec![7, 0, 0, 0x06, 7, 9, 0, 0];
        record.extend_from_slice(b"RicePaper");
        // SAFETY: `record` remains readable for this call.
        assert_eq!(
            unsafe {
                rice_wm_native_tasks_replace(record.as_ptr(), u32::try_from(record.len()).unwrap())
            },
            1,
        );
        assert_eq!(rice_wm_native_task_menu_show(7, 80, 96), 1);
        assert_eq!(rice_input_pointer(1, 1, 224, 584, 0), 1);
        assert_eq!(rice_wm_action_drain(), 0xfffa_0007);
        assert_eq!(rice_input_pointer(2, 1, 224, 584, 0), 1);

        assert_eq!(rice_input_pointer(1, 1, 650, 584, 0), 1);
        assert_eq!(rice_wm_current_workspace(), 1);
        assert_eq!(rice_wm_action_drain(), 0xfffb_0001);
        assert_eq!(rice_input_pointer(2, 1, 650, 584, 0), 1);
        assert_eq!(rice_wm_workspace_switch(2), 1);
        assert_eq!(rice_wm_current_workspace(), 2);
        assert_eq!(rice_wm_action_drain(), 0xfffb_0002);
        assert_eq!(rice_wm_workspace_switch(2), 0);
        assert_eq!(rice_wm_workspace_switch(4), 0);
        assert_eq!(
            // SAFETY: this deliberately exercises the explicit null check.
            unsafe { rice_wm_native_tasks_replace(std::ptr::null(), 1) },
            0,
        );
    }

    #[test]
    fn exported_abi_replaces_tray_plugins_and_launches_custom_app() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let record = [4, 25, 0, 0, 0x03, 0xff, 0xff, 0xff, 0xe3, 0x8f, 0x1a, 0];
        // SAFETY: `record` remains readable for this call.
        assert_eq!(
            unsafe { rice_wm_tray_replace(record.as_ptr(), u32::try_from(record.len()).unwrap()) },
            1,
        );
        assert_eq!(rice_input_pointer(1, 1, 780, 584, 0), 1);
        assert_eq!(rice_wm_action_drain(), 0xffff_ff03);
        assert_eq!(
            // SAFETY: this deliberately exercises the explicit null check.
            unsafe { rice_wm_tray_replace(std::ptr::null(), 12) },
            0,
        );
    }

    #[test]
    fn exported_abi_sets_connectivity_strategy_and_opens_preferences() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let record = [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        // SAFETY: `record` remains readable for this call.
        assert_eq!(
            unsafe { rice_wm_tray_replace(record.as_ptr(), u32::try_from(record.len()).unwrap()) },
            1,
        );
        assert_eq!(rice_wm_connectivity_set(3), 1);
        assert_eq!(rice_wm_connectivity_set(4), 0);
        assert_eq!(rice_input_pointer(1, 1, 780, 584, 0), 1);
        assert_eq!(rice_wm_action_drain(), 0xffff_ff0f);
    }

    #[test]
    fn exported_abi_shows_and_clears_native_notifications() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let title = b"LOCAL STORAGE CONNECTED";
        let message = b"PROJECT IS READY IN PCMANFM";
        // SAFETY: both byte slices remain readable for this call.
        assert_eq!(
            unsafe {
                rice_wm_notification_show(
                    1,
                    title.as_ptr(),
                    u32::try_from(title.len()).unwrap(),
                    message.as_ptr(),
                    u32::try_from(message.len()).unwrap(),
                )
            },
            1,
        );
        assert_ne!(rice_surface_geometry(0xe000_0004), RESULT_ERROR);
        assert_eq!(rice_wm_notification_clear(), 1);
        // SAFETY: this deliberately exercises the title validation path.
        assert_eq!(
            unsafe { rice_wm_notification_show(1, b"BAD\nTITLE".as_ptr(), 9, std::ptr::null(), 0) },
            0,
        );
    }

    #[test]
    fn exported_abi_replaces_and_launches_the_application_menu() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let label = b"TEXT EDITOR";
        let mut record = Vec::from(42u32.to_le_bytes());
        record.extend([7, u8::try_from(label.len()).unwrap(), 1, 0]);
        record.extend(label);
        // SAFETY: `record` remains readable for the duration of this call.
        assert_eq!(
            unsafe { rice_wm_menu_replace(record.as_ptr(), u32::try_from(record.len()).unwrap(),) },
            1
        );
        assert_eq!(rice_input_pointer(1, 1, 18, 584, 0), 1);
        assert_eq!(rice_input_pointer(2, 1, 18, 584, 0), 1);
        assert_eq!(rice_wm_menu_dismiss(), 1);
        assert_eq!(rice_input_pointer(1, 1, 18, 584, 0), 1);
        assert_eq!(rice_input_pointer(2, 1, 18, 584, 0), 1);
        // The Windows-style cascade keeps the root, category and application
        // columns visible while navigating to the unchanged desktop entry.
        // Discover the bottom-anchored Applications row through the exported
        // geometry instead of coupling this ABI test to the root-row count.
        const WM_MENU_SURFACE: u32 = 0xe000_0002;
        let application_y = (0..568)
            .step_by(4)
            .find(|root_y| {
                assert_eq!(rice_input_pointer(0, 0, 18, *root_y, 0), 1);
                rice_surface_geometry(WM_MENU_SURFACE) & 0xffff > 230
            })
            .expect("hovering Applications opens the category column");
        assert_eq!(rice_input_pointer(0, 0, 250, application_y, 0), 1);
        assert_eq!(rice_input_pointer(1, 1, 500, application_y, 0), 1);
        assert_eq!(rice_input_pointer(2, 1, 500, application_y, 0), 1);
        assert_eq!(rice_wm_action_pending(), 1);
        assert_eq!(rice_wm_action_drain(), 42);
    }

    #[test]
    fn exported_v2_menu_accepts_bounded_filesystem_folder_paths() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let label = b"TEXT EDITOR";
        let path = b"My Tools\0Editors";
        let mut record = Vec::from(42u32.to_le_bytes());
        record.push(7);
        record.push(u8::try_from(label.len()).unwrap());
        record.extend(u16::try_from(path.len()).unwrap().to_le_bytes());
        // Provisionable + hydrating are retained independently from the
        // uninstallable bit used by ordinary installed applications.
        record.extend([6, 0]);
        record.extend(label);
        record.extend(path);
        assert_eq!(
            // SAFETY: `record` remains readable for the duration of this call.
            unsafe {
                rice_wm_menu_replace_v2(record.as_ptr(), u32::try_from(record.len()).unwrap())
            },
            1,
        );
    }

    #[test]
    fn exported_abi_accepts_only_bounded_data_only_themes() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(rice_init(800, 600), 1);
        let theme_record = |name: &[u8], backdrop_material: bool, chrome_profile: u8| {
            let name_header =
                u8::try_from(name.len()).unwrap() | if backdrop_material { 0x80 } else { 0 };
            let mut record = vec![name_header, 7 | (chrome_profile << 4), 6, 8];
            for color in 0..21u32 {
                record.extend_from_slice(&(0x0010_0000 + color).to_le_bytes());
            }
            record.extend_from_slice(name);
            record
        };
        let mut catalog = theme_record(b"Custom Safe Theme", false, 2);
        catalog[1] |= 0x08;
        catalog.extend(theme_record(b"Second Theme", true, 5));
        // SAFETY: `catalog` remains readable for the duration of the call.
        assert_eq!(
            unsafe {
                rice_wm_themes_replace(catalog.as_ptr(), u32::try_from(catalog.len()).unwrap())
            },
            1,
        );

        let injection = theme_record(b"<script>alert(1)</script>", false, 0);
        // SAFETY: `injection` remains readable for the duration of the call.
        assert_eq!(
            unsafe {
                rice_wm_themes_replace(injection.as_ptr(), u32::try_from(injection.len()).unwrap())
            },
            0,
        );
        let mut malformed = theme_record(b"Malformed", false, 0);
        malformed[1] = 0xf7;
        // SAFETY: `malformed` remains readable for the duration of the call.
        assert_eq!(
            unsafe {
                rice_wm_themes_replace(malformed.as_ptr(), u32::try_from(malformed.len()).unwrap())
            },
            0,
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_begin(enabled: u32) {let _=with_server(|s|s.begin_auto_tile(enabled!=0));}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_master(master: u32) {let _=with_server(|s|s.set_auto_tile_master(master!=0));}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_clip_count()->u32 {with_server(|s|s.auto_tile_clip_count()).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_clip_field(index:u32,field:u32)->u32 {with_server(|s|s.auto_tile_clip_field(index,field)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_fullscreen(id:u32) {if let Ok(id)=u16::try_from(id) {let _=with_server(|s|s.set_auto_tile_fullscreen(id));}}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_position(id:u32,x:i32,y:i32) {if let Ok(id)=u16::try_from(id) {let _=with_server(|s|s.set_auto_tile_position(id,x,y));}}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_clip_field(kind:u32,id:u32,field:u32)->u32 {with_server(|s|s.scrolling_clip_field(kind,id,field)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_scroll_tiles_at(delta:i32,x:i32,y:i32)->u32 {with_server(|s|u32::from(s.scroll_tiles_at(delta,x,y))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_scroll_drag_tiles_at(source:u32,delta:i32,x:i32,y:i32)->u32 {with_server(|s|u32::from(s.scroll_drag_tiles_at(source,delta,x,y))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_focus(id:u32) {if let Ok(id)=u16::try_from(id) {let _=with_server(|s|s.set_auto_tile_focus(id));}}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_scrolling(enabled:u32) {let _=with_server(|s|s.set_auto_tile_scrolling(enabled!=0));}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_scroll_tiles(delta:i32)->u32 {with_server(|s|u32::from(s.scroll_tiles(delta))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_snapshot(workspace:u32)->u32 {with_server(|s|s.capture_workspace_layout(workspace) as u32).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_word(index:u32)->u32 {with_server(|s|s.workspace_layout_word(index as usize)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_load_begin() {let _=with_server(|s|s.begin_workspace_layout_load());}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_load_word(word:u32)->u32 {with_server(|s|u32::from(s.push_workspace_layout_word(word))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_restore(workspace:u32)->u32 {with_server(|s|u32::from(s.restore_workspace_layout(workspace))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_identity(id:u32)->u32 {with_server(|s|s.prepare_layout_identity(id) as u32).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_identity_byte(index:u32)->u32 {with_server(|s|s.layout_identity_byte(index as usize)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_floating_field(id:u32,field:u32)->u32 {with_server(|s|s.layout_floating_field(id,field)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_layout_floating_restore(id:u32,x:i32,y:i32,width:u32,height:u32)->u32 {with_server(|s|u32::from(s.restore_layout_floating(id,x,y,width,height))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_navigation_begin() { let _=with_server(|s|s.navigation_begin()); }
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_keyboard_grabbed() -> u32 { with_server(|s|u32::from(s.keyboard_grabbed())).unwrap_or(0) }
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_pointer_grabbed() -> u32 {with_server(|s|u32::from(s.pointer_grabbed())).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_input_blocked() -> u32 {with_server(|s|u32::from(s.tile_input_blocked())).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_overflow() -> u32 {with_server(|s|u32::from(s.tile_overflow())).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_move_request_field(field:u32) -> u32 {with_server(|s|s.tile_move_request_field(field)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_move_request_clear() {with_server(|s|s.clear_tile_move_request());}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_pointer_release(x:i32,y:i32,state:u32) {
    with_server(|s|s.release_tile_pointer(x.clamp(i16::MIN as i32,i16::MAX as i32) as i16,y.clamp(i16::MIN as i32,i16::MAX as i32) as i16,state as u16));
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_navigation_native(id: u32, x: i32, y: i32, width: u32, height: u32) {
    if let Ok(id)=u16::try_from(id) { let _=with_server(|s|s.navigation_native_frame(id,x,y,width,height)); }
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_focus_direction(direction: u32) -> u32 { with_server(|s|s.focus_direction(direction)).unwrap_or(0) }
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_swap_direction(direction: u32) -> u32 { with_server(|s|u32::from(s.swap_direction(direction))).unwrap_or(0) }
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_control_available(kind: u32, id: u32, action: u32) -> u32 {
    with_server(|s|u32::from(s.tile_control_available(kind,id,action))).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_insert_direction(direction: u32) -> u32 {with_server(|s|u32::from(s.insert_direction(direction))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_insert_preview(source_kind: u32, source_id: u32, target_kind: u32, target_id: u32, direction: u32) -> u32 {
    with_server(|s|u32::from(s.preview_tile_insertion(source_kind,source_id,target_kind,target_id,direction))).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_insert_preview_field(field: u32) -> u32 {with_server(|s|s.tile_insertion_preview_field(field)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_insert(source_kind: u32, source_id: u32, target_kind: u32, target_id: u32, direction: u32) -> u32 {
    with_server(|s|u32::from(s.insert_tile(source_kind,source_id,target_kind,target_id,direction))).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_at_field(x:i32,y:i32,field:u32)->u32 {with_server(|s|s.tile_at_field(x,y,field)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tiled_x11_title_at(x:i32,y:i32)->u32 {with_server(|s|s.tiled_x11_title_at(x,y)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_activate_tiled_x11(id:u32)->u32 {with_server(|s|u32::from(s.activate_tiled_x11(id))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_resize_tile(operation: u32) -> u32 { with_server(|s|u32::from(s.resize_tile(operation))).unwrap_or(0) }
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_divider_at(x: i32, y: i32) -> u32 {with_server(|s|s.tile_divider_at(x,y)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_divider_position(token: u32) -> u32 {with_server(|s|s.tile_divider_position(token)).unwrap_or(u32::MAX)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_tile_divider_move(token: u32, position: i32) -> u32 {with_server(|s|u32::from(s.move_tile_divider(token,position))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_add(id: u32) {if let Ok(id)=u16::try_from(id){let _=with_server(|s|s.add_auto_tile_native(id));}}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_minimum(id: u32, width: u32, height: u32) {
    if let (Ok(id),Ok(width),Ok(height))=(u16::try_from(id),u16::try_from(width),u16::try_from(height)) {
        let _=with_server(|s|s.set_auto_tile_native_minimum(id,width,height));
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_apply() -> u32 {with_server(|s|s.apply_auto_tile() as u32).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_rect(index: u32, field: u32) -> u32 {with_server(|s|s.auto_tile_rect(index as usize,field as usize)).unwrap_or(0)}

/// Apply bounded topbar preferences or descriptive indicator metadata.
/// # Safety
/// The input must reference `len` readable bytes in Wasm memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_topbar_update(kind: u32, input: *const u8, len: u32) -> u32 {
    if len > 16384 || input.is_null() { return 0; }
    let bytes = unsafe { core::slice::from_raw_parts(input, len as usize) };
    let Ok(source) = core::str::from_utf8(bytes) else { return 0; };
    with_server(|server| u32::from(if kind == 0 { server.configure_topbar(source) } else { server.topbar_metadata(source) })).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_scrolling_workspaces(enabled:u32) {let _=with_server(|s|s.set_scrolling_workspaces(enabled!=0));}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_workspace_count()->u32 {with_server(|s|{s.refresh_scrolling_workspaces();s.workspace_count()}).unwrap_or(4)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_output_workspace(output:u32)->u32 {with_server(|s|s.output_workspace(output)).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_active_output()->u32 {with_server(|s|s.active_output()).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_workspace_list(output:u32,index:u32)->u32 {with_server(|s|s.workspace_list_field(output,index)).unwrap_or(u32::MAX)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_workspace_visible(kind:u32,id:u32,workspace:u32)->u32 {with_server(|s|u32::from(s.workspace_window_visible(kind,id,workspace))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_native_output_position(id:u32,x:i32,y:i32) {if id>0&&id<=65535{let _=with_server(|s|s.set_native_output_position(id as u16,x,y));}}

#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_auto_tile_workspace(workspace:u32) {let _=with_server(|s|s.set_auto_tile_workspace(workspace));}

/// Upload one application-owned notification icon; pixels are straight ARGB.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_notification_icon_update(slot:u32, tip_ptr:*const u8, tip_len:usize, width:u32, height:u32, pixels_ptr:*const u32, pixel_count:usize, hidden:u32) -> u32 {
    if tip_len>512 || (tip_len!=0 && tip_ptr.is_null()) || pixels_ptr.is_null() || width==0 || height==0 || width>128 || height>128 || pixel_count!=(width*height) as usize || hidden>1 {return 0;}
    let tip=if tip_len==0 {Vec::new()} else {unsafe {slice::from_raw_parts(tip_ptr,tip_len)}.to_vec()};
    let pixels=unsafe {slice::from_raw_parts(pixels_ptr,pixel_count)}.to_vec();
    with_server(|server|u32::from(server.update_notification_icon(slot,tip,width as u16,height as u16,pixels,hidden!=0))).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_notification_icon_remove(slot:u32)->u32 {with_server(|server|u32::from(server.remove_notification_icon(slot))).unwrap_or(0)}
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_notification_icon_focus(slot:u32)->u32 {with_server(|server|u32::from(server.focus_notification_icon(slot))).unwrap_or(0)}
/// Writes slot, native input message, signed screen x and y as four u32 words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_notification_event_drain(output:*mut u32)->u32 {
    if output.is_null() {return 0;}
    with_server(|server| {if let Some(event)=server.drain_notification_event() {unsafe {slice::from_raw_parts_mut(output,4)}.copy_from_slice(&event);1} else {0}}).unwrap_or(0)
}

/// Write edge,left,top,right,bottom,alwaysOnTop,autoHide,visible as eight i32 words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rice_wm_taskbar_geometry(output: *mut i32) -> u32 {
    if output.is_null() { return 0; }
    with_server(|server| {
        if let Some(geometry) = server.taskbar_geometry() {
            unsafe { slice::from_raw_parts_mut(output, 8) }.copy_from_slice(&geometry);
            1
        } else { 0 }
    }).unwrap_or(0)
}

/// Host-resolved locale code from the generated language registry.
#[unsafe(no_mangle)]
pub extern "C" fn rice_set_locale(locale: u32) -> u32 {
    if locale > u8::MAX as u32 || !rice_i18n::LOCALE_CODES.contains(&(locale as u8)) { return 0; }
    rice_i18n::set_locale(locale as u8);
    1
}

/// Bounded language pack ABI; the browser uploads only selected locales.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn rice_locale_catalog_loaded(locale: u32) -> u32 {
    u8::try_from(locale).is_ok_and(rice_i18n::browser_catalog::loaded) as u32
}
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn rice_locale_catalog_begin(locale: u32, bytes: u32) -> *mut u8 {
    rice_i18n::browser_catalog::begin(locale, bytes)
}
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn rice_locale_catalog_finish(locale: u32, bytes: u32) -> u32 {
    rice_i18n::browser_catalog::finish(locale, bytes) as u32
}
