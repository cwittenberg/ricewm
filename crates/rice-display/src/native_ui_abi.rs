//! Bounded Wasm façade joining RAPP application frames to native UI rendering.
//!
//! Input and output use the existing `rice_io_reserve` staging allocation, so
//! these exports never dereference an arbitrary caller-provided pointer.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

use rice_native_app_protocol::{
    AppId, MAX_MESSAGE_BYTES, MAX_TEXT_BYTES, Message, PointerEventKind, WidgetId, WindowId,
};
use rice_ui::{Constraints, Point, Rect, Size};

use crate::{
    RESULT_ERROR, io_buffer,
    native_ui::{DrawCommand, NativeUiHost},
    native_ui_snapshot::{MAX_SNAPSHOT_BYTES, encode_snapshot},
    native_ui_theme::{EXTENDED_THEME_RECORD_BYTES, decode_theme},
};

mod editor;

const MAX_RENDER_DIMENSION: u32 = 16_384;
const MAX_EVENT_FRAMES: usize = 256;
const MAX_EVENT_BYTES: usize = 4 * MAX_MESSAGE_BYTES;
const MAX_UI_EVENT_FRAME_BYTES: usize = MAX_TEXT_BYTES + 128;
const NATIVE_UI_ABI_VERSION: u32 = 1;

static TRANSPORT: OnceLock<Mutex<NativeUiTransport>> = OnceLock::new();

fn transport() -> &'static Mutex<NativeUiTransport> {
    TRANSPORT.get_or_init(|| Mutex::new(NativeUiTransport::default()))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct EventQueue {
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
}

impl EventQueue {
    fn ensure_room(&self) -> Result<(), TransportError> {
        self.ensure_room_for(1)
    }

    fn ensure_room_for(&self, count: usize) -> Result<(), TransportError> {
        if self.frames.len().saturating_add(count) > MAX_EVENT_FRAMES
            || self.bytes.saturating_add(MAX_UI_EVENT_FRAME_BYTES.saturating_mul(count)) > MAX_EVENT_BYTES
        {
            return Err(TransportError::QueueFull);
        }
        Ok(())
    }

    fn push(&mut self, message: &Message) -> Result<(), TransportError> {
        let frame = message.encode().map_err(|_| TransportError::Protocol)?;
        if self.frames.len() >= MAX_EVENT_FRAMES
            || self.bytes.saturating_add(frame.len()) > MAX_EVENT_BYTES
        {
            return Err(TransportError::QueueFull);
        }
        self.bytes += frame.len();
        self.frames.push_back(frame);
        Ok(())
    }

    fn push_size(&mut self, event: rice_native_app_protocol::UiEventEnvelope) -> Result<(), TransportError> {
        let key = (event.app, event.window, event.target);
        let frame = Message::UiEvent(event).encode().map_err(|_| TransportError::Protocol)?;
        if let Some(previous) = self.frames.iter_mut().find(|bytes| matches!(Message::decode(bytes), Ok(Message::UiEvent(e))
            if (e.app, e.window, e.target) == key && matches!(e.event, rice_native_app_protocol::UiEvent::SizeChanged { .. }))) {
            self.bytes = self.bytes - previous.len() + frame.len();
            *previous = frame;
            return Ok(());
        }
        self.push(&Message::decode(&frame).map_err(|_| TransportError::Protocol)?)
    }

    fn pending(&self) -> usize {
        self.frames.front().map_or(0, Vec::len)
    }

    fn drain(&mut self, destination: &mut [u8]) -> Result<usize, TransportError> {
        let Some(frame) = self.frames.front() else {
            return Ok(0);
        };
        if destination.len() < frame.len() {
            return Err(TransportError::OutputTooSmall);
        }
        let written = frame.len();
        destination[..written].copy_from_slice(frame);
        self.frames.pop_front();
        self.bytes -= written;
        Ok(written)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct NativeUiTransport {
    host: NativeUiHost,
    snapshot: Vec<u8>,
    selection: Vec<u8>,
    events: EventQueue,
    emitted_web_epochs: BTreeMap<(AppId, WindowId, rice_native_app_protocol::WidgetId), u64>,
    emitted_sizes: BTreeMap<(AppId, WindowId, WidgetId), Size>,
    editor_epoch: u64,
    editor_epochs: BTreeMap<(AppId, WindowId), (u64, u64)>,
    last_pointer_visual_changed: bool,
    last_pointer_motion_continuous: bool,
}

impl NativeUiTransport {
    fn forget_scene(&mut self, app: AppId, window: WindowId, surface: rice_native_app_protocol::WidgetId) {
        self.emitted_web_epochs.remove(&(app, window, surface));
    }

    fn set_theme(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.host
            .set_theme(decode_theme(bytes).ok_or(TransportError::Protocol)?);
        self.snapshot.clear();
        self.editor_epochs.clear();
        Ok(())
    }

    fn apply(&mut self, frame: &[u8]) -> Result<usize, TransportError> {
        match Message::decode(frame).map_err(|_| TransportError::Protocol)? {
            Message::UiPatch(envelope) => {
                if envelope.patches.iter().any(|patch| matches!(patch, rice_native_app_protocol::UiPatch::GridDamage { .. })) {
                    self.events.ensure_room()?;
                }
                self.host.apply_patch(&envelope).map_err(|_| TransportError::Ui)?;
                // Recreated roots and renewed subscriptions need a fresh size even at reused IDs.
                for patch in &envelope.patches {
                    use rice_native_app_protocol::UiPatch;
                    let reset = match patch { UiPatch::Clear => Some(None),
                        UiPatch::ObserveSize { id, .. } | UiPatch::Remove { id } => Some(Some(*id)), _ => None };
                    if let Some(id) = reset {
                        self.emitted_sizes.retain(|(a,w,t),_| *a != envelope.app || *w != envelope.window || id.is_some_and(|id| id != *t));
                        self.events.frames.retain(|bytes| !matches!(Message::decode(bytes), Ok(Message::UiEvent(e))
                            if e.app == envelope.app && e.window == envelope.window && id.is_none_or(|id| id == e.target)
                            && matches!(e.event, rice_native_app_protocol::UiEvent::SizeChanged { .. })));
                        self.events.bytes = self.events.frames.iter().map(Vec::len).sum();
                    }
                    let reset_selection = match patch { UiPatch::Clear => Some(None),
                        UiPatch::EditSelection { id, .. } | UiPatch::Remove { id } | UiPatch::Create { id, .. } => Some(Some(*id)), _ => None };
                    if let Some(id) = reset_selection {
                        self.events.frames.retain(|bytes| !matches!(Message::decode(bytes), Ok(Message::UiEvent(e))
                            if e.app == envelope.app && e.window == envelope.window && id.is_none_or(|id| id == e.target)
                            && matches!(e.event, rice_native_app_protocol::UiEvent::TextSelection { .. })));
                    }
                }
                self.emitted_sizes.retain(|(a,w,t),_| self.host.observes_widget_size(*a,*w,*t));
                self.events.frames.retain(|bytes| !matches!(Message::decode(bytes), Ok(Message::UiEvent(e))
                    if matches!(e.event, rice_native_app_protocol::UiEvent::SizeChanged { .. })
                    && !self.host.observes_widget_size(e.app,e.window,e.target)));
                // TextEditor keeps its existing event contract; remove metadata
                // for deleted field descendants as well as explicitly removed IDs.
                self.events.frames.retain(|bytes| !matches!(Message::decode(bytes), Ok(Message::UiEvent(e))
                    if matches!(e.event, rice_native_app_protocol::UiEvent::TextSelection { .. })
                    && !self.host.accepts_text_selection(e.app,e.window,e.target)));
                self.events.bytes = self.events.frames.iter().map(Vec::len).sum();
                if let Some(target) = self.host.take_grid_resync(envelope.app, envelope.window) {
                    self.events.push(&Message::UiEvent(rice_native_app_protocol::UiEventEnvelope {
                        app: envelope.app, window: envelope.window, target,
                        event: rice_native_app_protocol::UiEvent::GridResync,
                    }))?;
                }
            },
            Message::WebRenderPatch(patch) => {
                self.host.apply_web_render(&patch).map_err(|_| TransportError::Ui)?;
                // A removed/recreated surface may restart its epoch. Every
                // accepted replacement owns fresh bytes, even at a reused ID.
                self.emitted_web_epochs.remove(&(patch.app, patch.window, patch.surface));
            }
            _ => return Err(TransportError::WrongMessage),
        }
        Ok(frame.len())
    }

    fn render(
        &mut self,
        app: AppId,
        window: WindowId,
        width: u32,
        height: u32,
    ) -> Result<usize, TransportError> {
        if width == 0
            || height == 0
            || width > MAX_RENDER_DIMENSION
            || height > MAX_RENDER_DIMENSION
        {
            return Err(TransportError::Dimensions);
        }
        let size = Size { width, height };
        let constraints = Constraints::new(size, size).ok_or(TransportError::Dimensions)?;
        let mut output = self
            .host
            .render(app, window, constraints)
            .map_err(|_| TransportError::Ui)?;
        let mut emitted = Vec::new();
        for command in &mut output.commands {
            if let DrawCommand::WebScene {
                surface,
                epoch,
                display_list,
                ..
            } = command
            {
                let key = (app, window, *surface);
                if self.emitted_web_epochs.get(&key) == Some(epoch) {
                    *display_list = Arc::default();
                } else if *epoch != 0 {
                    emitted.push((key, *epoch));
                }
            }
        }
        let snapshot =
            encode_snapshot(app.get(), window.get(), &output).map_err(|_| TransportError::Ui)?;
        if snapshot.len() > MAX_SNAPSHOT_BYTES {
            return Err(TransportError::Ui);
        }
        let length = snapshot.len();
        self.snapshot = snapshot;
        self.editor_epoch = self.editor_epoch.checked_add(1).ok_or(TransportError::Ui)?;
        if self.editor_epochs.len() >= 64 && !self.editor_epochs.contains_key(&(app, window)) {
            if let Some(key) = self.editor_epochs.keys().next().copied() { self.editor_epochs.remove(&key); }
        }
        self.editor_epochs.insert((app, window), (self.editor_epoch, self.host.sequence(app, window).ok_or(TransportError::Ui)?));
        self.emitted_web_epochs.extend(emitted);
        let sizes = self.host.observed_widget_sizes(app, window);
        self.emitted_sizes.retain(|(a,w,t),_| *a != app || *w != window || sizes.iter().any(|(id,_)| id == t));
        for (target, size) in sizes {
            let key = (app, window, target);
            if self.emitted_sizes.get(&key) == Some(&size) { continue; }
            if self.events.push_size(rice_native_app_protocol::UiEventEnvelope { app, window, target,
                event: rice_native_app_protocol::UiEvent::SizeChanged { width: size.width, height: size.height } }).is_ok() {
                self.emitted_sizes.insert(key, size);
            }
        }
        Ok(length)
    }

    fn pointer(
        &mut self,
        address: Address,
        kind: PointerEventKind,
        point: Point,
        button: u8,
        modifiers: u16,
    ) -> Result<bool, TransportError> {
        self.pointer_measured(address, kind, point, button, modifiers, None)
    }

    fn pointer_measured(
        &mut self, address: Address, kind: PointerEventKind, point: Point,
        button: u8, modifiers: u16,
        measured: Option<&crate::native_ui::editor_measurement::EditorMeasurement>,
    ) -> Result<bool, TransportError> {
        if measured.is_some_and(|sample| !self.host.validate_editor_measurement(address.app, address.window, kind, point, sample)) {
            return Err(TransportError::Ui);
        }
        self.events.ensure_room_for(if self.host.has_text_selection_observers(address.app, address.window) { 2 } else { 1 })?;
        let selection_before = self.host.text_field_selection(address.app, address.window);
        self.last_pointer_visual_changed = false;
        let before = self.host.pointer_visual_state(address.app, address.window);
        let event = self
            .host
            .pointer_event_measured(address.app, address.window, kind, point, button, modifiers, measured)
            .map_err(|_| TransportError::Ui)?;
        self.last_pointer_visual_changed =
            before != self.host.pointer_visual_state(address.app, address.window);
        self.last_pointer_motion_continuous = self
            .host
            .pointer_motion_continuous(address.app, address.window);
        if let Some(event) = event {
            self.events.push(&Message::UiEvent(event))?;
            let selection = self.host.text_field_selection(address.app, address.window);
            if selection != selection_before && let Some(selection) = selection {
                self.events.push(&Message::UiEvent(selection))?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn activate(&mut self, address: Address, target: WidgetId) -> Result<bool, TransportError> {
        self.events.ensure_room()?;
        let event = self
            .host
            .activate_event(address.app, address.window, target)
            .map_err(|_| TransportError::Ui)?;
        if let Some(event) = event {
            self.events.push(&Message::UiEvent(event))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn key(
        &mut self,
        address: Address,
        pressed: bool,
        key_code: u32,
        modifiers: u16,
        text: Option<String>,
    ) -> Result<bool, TransportError> {
        self.events.ensure_room_for(if self.host.has_text_selection_observers(address.app, address.window) { 2 } else { 1 })?;
        let selection_before = self.host.text_field_selection(address.app, address.window);
        let event = self
            .host
            .key_event(
                address.app,
                address.window,
                pressed,
                key_code,
                modifiers,
                text,
            )
            .map_err(|_| TransportError::Ui)?;
        if let Some(event) = event {
            let mut events = self.events.clone();
            if let Some(scroll) = self.host.navigation_scroll_event(&event) {
                events.push(&Message::UiEvent(scroll))?;
            }
            let value_changed = matches!(event.event, rice_native_app_protocol::UiEvent::ValueChanged(_));
            let edited_value = match &event.event { rice_native_app_protocol::UiEvent::ValueChanged(value) => Some(value.as_str()), _ => None };
            let selection = self.host.text_field_selection_for_value(address.app, address.window, edited_value);
            events.push(&Message::UiEvent(event))?;
            if (value_changed || selection != selection_before) && let Some(selection) = selection {
                events.push(&Message::UiEvent(selection))?;
            }
            self.events = events;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn hit_test(&self, address: Address, point: Point) -> Option<u64> {
        self.host
            .hit_test(address.app, address.window, point)
            .map(|target| target.widget.get())
    }

    fn prepare_selection(&mut self, address: Address) -> usize {
        self.selection = self
            .host
            .selected_text(address.app, address.window)
            .unwrap_or_default()
            .into_bytes();
        self.selection.len()
    }
}

#[derive(Clone, Copy)]
struct Address {
    app: AppId,
    window: WindowId,
}

impl Address {
    fn from_halves(app_lo: u32, app_hi: u32, window_lo: u32, window_hi: u32) -> Option<Self> {
        let app = u64::from(app_lo) | (u64::from(app_hi) << 32);
        let window = u64::from(window_lo) | (u64::from(window_hi) << 32);
        Some(Self {
            app: AppId::new(app)?,
            window: WindowId::new(window)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportError {
    Protocol,
    WrongMessage,
    Ui,
    Dimensions,
    QueueFull,
    OutputTooSmall,
}

fn staging_input(length: u32, maximum: usize) -> Result<Vec<u8>, TransportError> {
    let length = usize::try_from(length).map_err(|_| TransportError::Protocol)?;
    if length == 0 || length > maximum {
        return Err(TransportError::Protocol);
    }
    let staging = io_buffer().lock().map_err(|_| TransportError::Protocol)?;
    staging
        .get(..length)
        .map(<[u8]>::to_vec)
        .ok_or(TransportError::Protocol)
}

fn pointer_kind(value: u32) -> Option<PointerEventKind> {
    match value {
        0 => Some(PointerEventKind::Move),
        1 => Some(PointerEventKind::Down),
        2 => Some(PointerEventKind::Up),
        3 => Some(PointerEventKind::Enter),
        4 => Some(PointerEventKind::Leave),
        5 => Some(PointerEventKind::Wheel),
        _ => None,
    }
}

/// Return the native UI transport ABI version.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_abi_version() -> u32 {
    NATIVE_UI_ABI_VERSION
}

/// Reset every retained native window, draw snapshot, and pending input event.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_init() -> u32 {
    let Ok(mut transport) = transport().lock() else {
        return RESULT_ERROR;
    };
    *transport = NativeUiTransport::default();
    1
}

/// Forget a display-backend payload acknowledgement without releasing the owned
/// scene. Its next snapshot retransmits the bytes after backend cache eviction.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_forget_scene(
    app_lo: u32, app_hi: u32, window_lo: u32, window_hi: u32, surface_lo: u32, surface_hi: u32,
) -> u32 {
    let Some(app) = AppId::new(u64::from(app_lo) | (u64::from(app_hi) << 32)) else { return RESULT_ERROR; };
    let Some(window) = WindowId::new(u64::from(window_lo) | (u64::from(window_hi) << 32)) else { return RESULT_ERROR; };
    let Some(surface) = rice_native_app_protocol::WidgetId::new(u64::from(surface_lo) | (u64::from(surface_hi) << 32)) else { return RESULT_ERROR; };
    let Ok(mut transport) = transport().lock() else { return RESULT_ERROR; };
    transport.forget_scene(app, window, surface);
    1
}

/// Release retained UI resources after an application has stopped emitting frames.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_remove_app(app_lo: u32, app_hi: u32) -> u32 {
    let Some(app) = AppId::new(u64::from(app_lo) | (u64::from(app_hi) << 32)) else {
        return RESULT_ERROR;
    };
    let Ok(mut transport) = transport().lock() else { return RESULT_ERROR; };
    transport.host.remove_app(app);
    transport.emitted_sizes.retain(|(owner, _, _), _| *owner != app);
    transport.emitted_web_epochs.retain(|(owner, _, _), _| *owner != app);
    transport.events.frames.retain(|frame| {
        !matches!(Message::decode(frame), Ok(Message::UiEvent(event)) if event.app == app)
    });
    transport.events.bytes = transport.events.frames.iter().map(Vec::len).sum();
    transport.snapshot.clear();
    transport.selection.clear();
    1
}

/// Feature probe; legacy display modules have no such export.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_damage_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_fill_endpoint_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_text_style_version() -> u32 { 1 }

/// Resolved widget-size observation patch and render feedback support.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_widget_size_version() -> u32 { 1 }

/// Large bounded VirtualGrid snapshots.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_large_frame_version() -> u32 { 1 }

/// Filter buttons positioned on a logical header row.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_filter_row_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_references_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_text_field_selection_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_document_font_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_paint_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_zoom_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_document_canvas_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_fractional_font_version() -> u32 { 1 }

/// Additive semantic zoom icons; older hosts receive SDK text fallbacks.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_zoom_icons_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_grid_merges_version() -> u32 { 1 }

/// Optional native document editor alignment; legacy fields stay centered.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_document_alignment_version() -> u32 { 1 }

/// Read preferred geometry of a bounded detached SDK tree. Only creates are
/// accepted and the temporary host is discarded without publishing any window.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_measure_tree(input_len:u32,width:u32,app_lo:u32,app_hi:u32,window_lo:u32,window_hi:u32)->u64 {
    if width!=0 && !(96..=4096).contains(&width) {return 0;}
    let Ok(frame)=staging_input(input_len,256*1024) else {return 0;};
    let Ok(Message::UiPatch(envelope))=Message::decode(&frame) else {return 0;};
    if envelope.sequence!=1 || envelope.patches.is_empty() || envelope.patches.len()>2048
        || envelope.patches.iter().any(|patch| !matches!(patch,rice_native_app_protocol::UiPatch::Create{..}))
        || envelope.patches.iter().filter(|patch| matches!(patch,rice_native_app_protocol::UiPatch::Create{parent:None,..})).count()!=1 {return 0;}
    let owner=Address::from_halves(app_lo,app_hi,window_lo,window_hi).map(|address|(address.app,address.window));
    transport().lock().ok().and_then(|transport|transport.host.measure_detached(&envelope,width,owner).ok())
        .map_or(0,|size| u64::from(size.width) | (u64::from(size.height)<<32))
}

/// Decode and atomically apply exactly one staged RAPP `UiPatch` frame.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_apply(input_len: u32) -> u32 {
    let Ok(frame) = staging_input(input_len, MAX_MESSAGE_BYTES) else {
        return RESULT_ERROR;
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| transport.apply(&frame).ok())
        .and_then(|consumed| u32::try_from(consumed).ok())
        .unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_icon_selector_version()->u32 {1}

/// Version two adds bounded typography and native control geometry overrides.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_theme_version() -> u32 { 2 }

/// Apply one staged, data-only `RiceWM` catalog theme to every native app.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_set_theme(input_len: u32) -> u32 {
    let Ok(frame) = staging_input(input_len, EXTENDED_THEME_RECORD_BYTES) else {
        return RESULT_ERROR;
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| transport.set_theme(&frame).ok())
        .map_or(RESULT_ERROR, |()| 1)
}

/// Supply bounded desktop work-area geometry for owned floating editors.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_dialog_work_area(
    app_lo: u32, app_hi: u32, window_lo: u32, window_hi: u32,
    x: i32, y: i32, width: u32, height: u32,
) -> u32 {
    let Some(address) = Address::from_halves(app_lo, app_hi, window_lo, window_hi) else { return RESULT_ERROR; };
    if width > MAX_RENDER_DIMENSION || height > MAX_RENDER_DIMENSION
        || x.unsigned_abs() > MAX_RENDER_DIMENSION || y.unsigned_abs() > MAX_RENDER_DIMENSION { return RESULT_ERROR; }
    let area = (width > 0 && height > 0).then_some(Rect { origin: Point { x, y }, size: Size { width, height } });
    transport().lock().ok().map_or(RESULT_ERROR, |mut transport| {
        if transport.host.set_dialog_work_area(address.app, address.window, area) { 1 } else { RESULT_ERROR }
    })
}

/// Render one retained window and retain its complete semantic draw snapshot.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_render(
    app_lo: u32,
    app_hi: u32,
    window_lo: u32,
    window_hi: u32,
    width: u32,
    height: u32,
) -> u32 {
    let Some(address) = Address::from_halves(app_lo, app_hi, window_lo, window_hi) else {
        return RESULT_ERROR;
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| {
            transport
                .render(address.app, address.window, width, height)
                .ok()
        })
        .and_then(|length| u32::try_from(length).ok())
        .unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_snapshot_pending() -> u32 {
    transport()
        .lock()
        .ok()
        .and_then(|transport| u32::try_from(transport.snapshot.len()).ok())
        .unwrap_or(RESULT_ERROR)
}

/// Copy the complete draw snapshot to the staged region without consuming it.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_snapshot_drain(capacity: u32) -> u32 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return RESULT_ERROR;
    };
    let Ok(mut staging) = io_buffer().lock() else {
        return RESULT_ERROR;
    };
    let Some(destination) = staging.get_mut(..capacity) else {
        return RESULT_ERROR;
    };
    let Ok(transport) = transport().lock() else {
        return RESULT_ERROR;
    };
    if transport.snapshot.len() > destination.len() {
        return RESULT_ERROR;
    }
    destination[..transport.snapshot.len()].copy_from_slice(&transport.snapshot);
    u32::try_from(transport.snapshot.len()).unwrap_or(RESULT_ERROR)
}

/// Hit-test pointer input and queue one complete RAPP `UiEvent` when targeted.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_pointer(
    app_lo: u32,
    app_hi: u32,
    window_lo: u32,
    window_hi: u32,
    kind: u32,
    x: i32,
    y: i32,
    button: u32,
    modifiers: u32,
) -> u32 {
    let (Some(address), Some(kind), Ok(button), Ok(modifiers)) = (
        Address::from_halves(app_lo, app_hi, window_lo, window_hi),
        pointer_kind(kind),
        u8::try_from(button),
        u16::try_from(modifiers),
    ) else {
        return RESULT_ERROR;
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| {
            transport
                .pointer(address, kind, Point { x, y }, button, modifiers)
                .ok()
        })
        .map_or(RESULT_ERROR, u32::from)
}

/// Report whether the preceding pointer call changed host-owned visual state.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_pointer_visual_changed() -> u32 {
    transport().lock().map_or(0, |transport| {
        u32::from(transport.last_pointer_visual_changed)
    })
}

/// Report whether the preceding pointer target consumes continuous motion.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_pointer_motion_continuous() -> u32 {
    transport().lock().map_or(0, |transport| {
        u32::from(transport.last_pointer_motion_continuous)
    })
}

/// Queue one semantic activation for an exact retained widget.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_activate(
    app_lo: u32,
    app_hi: u32,
    window_lo: u32,
    window_hi: u32,
    target_lo: u32,
    target_hi: u32,
) -> u32 {
    let target = u64::from(target_lo) | (u64::from(target_hi) << 32);
    let (Some(address), Some(target)) = (
        Address::from_halves(app_lo, app_hi, window_lo, window_hi),
        WidgetId::new(target),
    ) else {
        return RESULT_ERROR;
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| transport.activate(address, target).ok())
        .map_or(RESULT_ERROR, u32::from)
}

/// Read operation flags for an exact retained widget without mutating UI state.
/// Bits: 1 activation, 2 editable text, 4 focus. Zero means unsupported/absent.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_widget_actions(
    app_lo:u32, app_hi:u32, window_lo:u32, window_hi:u32, target_lo:u32, target_hi:u32,
) -> u32 {
    let target=u64::from(target_lo)|(u64::from(target_hi)<<32);
    let (Some(address),Some(target))=(Address::from_halves(app_lo,app_hi,window_lo,window_hi),WidgetId::new(target)) else { return 0; };
    transport().lock().map_or(0,|transport|transport.host.widget_actions(address.app,address.window,target))
}

/// Return the semantic widget under a window-local point, or zero for none.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_hit_test(
    app_lo: u32,
    app_hi: u32,
    window_lo: u32,
    window_hi: u32,
    x: i32,
    y: i32,
) -> u64 {
    let Some(address) = Address::from_halves(app_lo, app_hi, window_lo, window_hi) else {
        return 0;
    };
    transport()
        .lock()
        .ok()
        .and_then(|transport| transport.hit_test(address, Point { x, y }))
        .unwrap_or(0)
}

/// Read the owned overflowing scroll viewport at a window-local point.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_scroll_owner(
    app_lo:u32, app_hi:u32, window_lo:u32, window_hi:u32, x:i32, y:i32,
) -> u64 {
    let Some(address)=Address::from_halves(app_lo,app_hi,window_lo,window_hi) else {return 0;};
    transport().lock().ok().and_then(|transport|
        transport.host.scroll_owner_at_point(address.app,address.window,Point{x,y})).map_or(0, WidgetId::get)
}

/// Route key input at host focus; optional UTF-8 text is read from staging.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_key(
    app_lo: u32,
    app_hi: u32,
    window_lo: u32,
    window_hi: u32,
    pressed: u32,
    key_code: u32,
    modifiers: u32,
    text_len: u32,
) -> u32 {
    let (Some(address), Ok(modifiers)) = (
        Address::from_halves(app_lo, app_hi, window_lo, window_hi),
        u16::try_from(modifiers),
    ) else {
        return RESULT_ERROR;
    };
    if pressed > 1 {
        return RESULT_ERROR;
    }
    let text = if text_len == 0 {
        None
    } else {
        let Ok(bytes) = staging_input(text_len, MAX_TEXT_BYTES) else {
            return RESULT_ERROR;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return RESULT_ERROR;
        };
        Some(text)
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| {
            transport
                .key(address, pressed == 1, key_code, modifiers, text)
                .ok()
        })
        .map_or(RESULT_ERROR, u32::from)
}

/// Prepare the selected text from the focused standard text field.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_selection_prepare(
    app_lo: u32,
    app_hi: u32,
    window_lo: u32,
    window_hi: u32,
) -> u32 {
    let Some(address) = Address::from_halves(app_lo, app_hi, window_lo, window_hi) else {
        return RESULT_ERROR;
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| u32::try_from(transport.prepare_selection(address)).ok())
        .unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_selection_pending() -> u32 {
    transport()
        .lock()
        .ok()
        .and_then(|transport| u32::try_from(transport.selection.len()).ok())
        .unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_selection_drain(capacity: u32) -> u32 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return RESULT_ERROR;
    };
    let Ok(mut transport) = transport().lock() else {
        return RESULT_ERROR;
    };
    if capacity != transport.selection.len() {
        return RESULT_ERROR;
    }
    if capacity == 0 {
        transport.selection.clear();
        return 0;
    }
    let Ok(mut staging) = io_buffer().lock() else {
        return RESULT_ERROR;
    };
    let Some(destination) = staging.get_mut(..capacity) else {
        return RESULT_ERROR;
    };
    destination.copy_from_slice(&transport.selection);
    transport.selection.clear();
    u32::try_from(capacity).unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_event_pending() -> u32 {
    transport()
        .lock()
        .ok()
        .and_then(|transport| u32::try_from(transport.events.pending()).ok())
        .unwrap_or(RESULT_ERROR)
}

/// Drain exactly one complete RAPP `UiEvent` frame into the staged region.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_event_drain(capacity: u32) -> u32 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return RESULT_ERROR;
    };
    let Ok(mut staging) = io_buffer().lock() else {
        return RESULT_ERROR;
    };
    let Some(destination) = staging.get_mut(..capacity) else {
        return RESULT_ERROR;
    };
    transport()
        .lock()
        .ok()
        .and_then(|mut transport| transport.events.drain(destination).ok())
        .and_then(|written| u32::try_from(written).ok())
        .unwrap_or(RESULT_ERROR)
}


#[cfg(test)]
#[path = "native_ui_scene_abi_tests.rs"]
mod scene_tests;

#[cfg(test)]
#[path = "native_ui_size_tests.rs"]
mod size_tests;

#[cfg(test)]
#[path = "native_ui_text_selection_tests.rs"]
mod text_selection_tests;

/// Semantic spreadsheet formatting/data icon set.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_sheet_data_icons_version() -> u32 { 1 }

/// Content-alpha shadows with optional independent affine transform.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_content_shadows_version() -> u32 { 1 }

/// Select shared adaptive layout without changing desktop application themes.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_form_factor(compact: u32) -> u32 {
    let Ok(mut transport) = transport().lock() else { return RESULT_ERROR; };
    transport.host.compact_form_factor = compact != 0;
    1
}

/// Scoped native label/button typography, independently negotiated.
#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_control_typography_version() -> u32 { 1 }
