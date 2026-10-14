//! Bounded shared layout hints carried by container metadata.

use rice_ui::{Point, Rect, Size, Theme, Widget, WidgetKind, WidgetState};

const CLAMP_PREFIX: &str = "panel:clamp:";
const DISTRIBUTE: &str = "panel:distribute";
const NAVIGATION_ROW: &str = "panel:navigation-row";
const EQUAL_FIXED: &str = "panel:equal-fixed";
const ACTIONS: &str = "panel:actions";
const STATUS_BAR: &str = "panel:status-bar";
const SIDE_SHEET_PREFIX: &str = "panel:side-sheet";
const SPIN_CONTROL: &str = "panel:spin-control";
const SPIN_BUTTONS: &str = "panel:spin-buttons";
const ZOOM_PREFIX: &str = "zoom:";
const FILE_GRID_PREFIX: &str = "file-grid:";
const THUMBNAIL_CELL: &str = "panel:thumbnail-cell";
const DESKTOP_THUMBNAIL_CELL: &str = "desktop:thumbnail-cell";
const APP_GRID: &str = "app-grid";
const VIEWPORT_PREFIX: &str = "viewport:";
const FIXED_HEIGHT_PREFIX: &str = "panel:height:";
const FIXED_PANEL_HEIGHT_PREFIX: &str = "panel:fixed-height:";
const TRAILING_ACTION: &str = "panel:trailing-action";
const CALENDAR_DAY_BAND_PREFIX: &str = "panel:calendar-day-band-";
const WELCOME_FEATURES: &str = "panel:welcome-features";
const MIN_CLAMP: u32 = 240;
const MAX_CLAMP: u32 = 1_600;
const TABLE_COLUMNS_PREFIX: &str = "table-columns:";
pub(super) const DENSE_ROW_HEIGHT: u32 = 36;
pub(super) const DENSE_HEADER_HEIGHT: u32 = 20;

/// Explicit container inset in logical pixels. Flush is for internal structure;
/// visible reading surfaces can retain a consistent, bounded inner margin.
pub(super) fn padding(text: Option<&str>) -> Option<u32> {
    text?.split(':').find_map(|part| part.strip_prefix("padding=")?.parse::<u32>().ok().map(|value| value.min(64)))
}

pub(super) fn clamped_content(text: Option<&str>, content: Rect) -> Rect {
    if text == Some(WELCOME_FEATURES) {
        return Rect {
            origin: content.origin,
            size: Size {
                width: content.size.width.min(1_050),
                height: content.size.height,
            },
        };
    }
    let Some(maximum) = text.and_then(parse_clamp) else {
        return content;
    };
    let width = content.size.width.min(maximum);
    Rect {
        origin: Point {
            x: content
                .origin
                .x
                .saturating_add_unsigned(content.size.width.saturating_sub(width) / 2),
            y: content.origin.y,
        },
        size: Size {
            width,
            height: content.size.height,
        },
    }
}

pub(super) fn distributes_children(text: Option<&str>) -> bool {
    matches!(text, Some(DISTRIBUTE | NAVIGATION_ROW))
}

pub(super) fn is_navigation_row(text: Option<&str>) -> bool {
    text == Some(NAVIGATION_ROW)
}

pub(super) fn is_side_sheet(text: Option<&str>) -> bool {
    text.is_some_and(|text| {
        text == SIDE_SHEET_PREFIX
            || text
                .strip_prefix(SIDE_SHEET_PREFIX)
                .is_some_and(|suffix| suffix.starts_with(':'))
    })
}

pub(super) fn side_sheet_title(text: Option<&str>) -> Option<&str> {
    text?.strip_prefix("panel:side-sheet:")?.split(':').next()
}

pub(super) fn equal_fixed(text: Option<&str>) -> bool {
    has_token(text, EQUAL_FIXED.trim_start_matches("panel:"))
}

pub(super) fn equal_row(text: Option<&str>) -> bool {
    matches!(text, Some("panel:equal") | Some(WELCOME_FEATURES))
}

/// Relative widths for the direct cells of every row in a table. The values
/// are deliberately bounded metadata rather than pixels so a table keeps its
/// proportions when its window is resized.
pub(super) fn table_columns(text: Option<&str>) -> Option<Vec<u16>> {
    let values = text?
        .strip_prefix(TABLE_COLUMNS_PREFIX)?
        .split(':')
        .next()?
        .split(',')
        .map(str::parse::<u16>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() >= 2
        && values.len() <= 32
        && values.iter().all(|value| (10..=1_000).contains(value))
        && values.iter().copied().map(u32::from).sum::<u32>() <= 4_000)
        .then_some(values)
}

pub(super) fn resizable_table(text: Option<&str>) -> bool {
    table_columns(text).is_some() && has_token(text, "resizable")
}

pub(super) fn is_spacer(text: Option<&str>) -> bool {
    text == Some("panel:spacer")
}

pub(super) fn is_file_detail_disclosure_spacer(text: Option<&str>) -> bool {
    has_token(text, "detail-disclosure-spacer")
}

pub(super) fn is_file_detail_disclosure(text: Option<&str>) -> bool {
    has_token(text, "detail-disclosure") && !is_file_detail_disclosure_spacer(text)
}

pub(super) fn is_flexible(text: Option<&str>) -> bool {
    has_token(text, "flex")
}

pub(super) fn is_intrinsic_stack(text: Option<&str>) -> bool {
    has_token(text, "tight-stack")
}

pub(super) fn is_intrinsic_flow(text: Option<&str>) -> bool {
    has_token(text, "intrinsic-flow")
}

pub(super) fn is_store_header(text: Option<&str>) -> bool {
    has_token(text, "store-header")
}

pub(super) fn is_store_document(text: Option<&str>) -> bool {
    has_token(text, "store-document")
}

pub(super) fn is_filter_strip(text: Option<&str>) -> bool {
    has_token(text, "filter-strip")
}

pub(super) fn child_gap(text: Option<&str>, normal: u32) -> u32 {
    if let Some(gap) = text.and_then(|text| text.strip_prefix("panel:"))
        .and_then(|text| text.split(':').find_map(|part| part.strip_prefix("gap=")))
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|gap| *gap <= 64)
    {
        return gap;
    }
    if has_token(text, "dense-table") || is_browser_shell(text) || is_integrated_tabs(text) {
        0
    } else if has_token(text, "resource-timeline") {
        0
    } else if is_browser_tab_strip(text) {
        2
    } else if text == Some("panel:calendar-grid") {
        0
    } else if text == Some("panel:brand-row") {
        0
    } else if text == Some(WELCOME_FEATURES) {
        24
    } else if text == Some("panel:welcome-actions") {
        20
    } else if text == Some("panel:feature-card-content") {
        2
    } else if has_token(text, "bookmark") {
        2
    } else if text == Some("panel:accordion-stack") {
        10
    } else if text == Some(SPIN_BUTTONS) {
        2
    } else if text == Some("panel:tip-stack") {
        normal.saturating_mul(3)
    } else if is_section_stack(text) {
        normal.saturating_mul(2)
    } else if is_settings_card(text) {
        normal
    } else if is_settings_shell(text) {
        normal
    } else if is_intrinsic_stack(text) {
        2
    } else {
        normal
    }
}

pub(super) fn aligns_actions_end(text: Option<&str>) -> bool {
    text.is_some_and(|text| text == ACTIONS || text.starts_with("panel:actions:"))
}

pub(super) fn centers_setting_choice_control(text: Option<&str>) -> bool {
    has_token(text, "setting-choice")
}

pub(super) fn centers_children(text: Option<&str>) -> bool {
    text == Some("panel:welcome-actions")
}

pub(super) fn centers_vertical(text: Option<&str>) -> bool {
    text.is_some_and(|text| text.split(':').any(|part| part == "center-content"))
}

pub(super) fn is_settings_page(text: Option<&str>) -> bool {
    has_token(text, "settings-page")
}

pub(super) fn is_settings_card(text: Option<&str>) -> bool {
    has_token(text, "settings-card")
}

pub(super) fn is_settings_shell(text: Option<&str>) -> bool {
    has_token(text, "settings-shell")
}

pub(super) fn is_fixed_panel(text: Option<&str>) -> bool {
    has_token(text, "fixed") || is_status_bar(text)
}

pub(super) fn is_status_bar(text: Option<&str>) -> bool {
    has_token(text, STATUS_BAR.trim_start_matches("panel:"))
}

pub(super) fn is_spin_control(text: Option<&str>) -> bool {
    text.is_some_and(|text| {
        text.split(':')
            .any(|part| part == SPIN_CONTROL.trim_start_matches("panel:"))
    })
}

pub(super) fn is_spin_buttons(text: Option<&str>) -> bool {
    text.is_some_and(|text| text.split(':').any(|part| part == "spin-buttons"))
}

pub(super) fn is_tab_composite(text: Option<&str>) -> bool {
    has_token(text, "tab")
}

pub(super) fn is_vertical_tabs(text: Option<&str>) -> bool {
    has_token(text, "tabs") && has_token(text, "vertical")
}

pub(super) fn is_integrated_tabs(text: Option<&str>) -> bool {
    has_token(text, "tabs") && has_token(text, "integrated")
}

pub(super) fn is_tab_page(text: Option<&str>) -> bool {
    has_token(text, "tab-page")
}

pub(super) fn vertical_tabs_on_right(text: Option<&str>) -> bool {
    is_vertical_tabs(text) && has_token(text, "right")
}

pub(super) fn is_browser_tab(text: Option<&str>) -> bool {
    is_tab_composite(text) && has_token(text, "browser-tab")
}

pub(super) fn is_browser_shell(text: Option<&str>) -> bool {
    has_token(text, "browser-shell")
}

pub(super) fn is_browser_tab_strip(text: Option<&str>) -> bool {
    has_token(text, "browser-tab-strip")
}

pub(super) fn is_window_drag_region(text: Option<&str>) -> bool {
    has_token(text, "window-drag-region")
}

pub(super) fn is_window_control(text: Option<&str>) -> bool {
    has_token(text, "window-control")
}

pub(super) fn is_bookmark_composite(text: Option<&str>) -> bool {
    has_token(text, "bookmark")
}

pub(super) fn is_bookmark_bar(text: Option<&str>) -> bool {
    has_token(text, "bookmark-bar")
}

pub(super) fn is_reference_card(text: Option<&str>) -> bool {
    has_token(text, "reference-card")
}

pub(super) fn is_section_stack(text: Option<&str>) -> bool {
    has_token(text, "section-stack")
}

pub(super) fn compacts_children(text: Option<&str>) -> bool {
    text.is_some_and(|text| text.split(':').any(|part| part == "compact"))
}

pub(super) fn is_dense_row(text: Option<&str>) -> bool {
    has_token(text, "dense")
}

pub(super) fn dense_row_height(text: Option<&str>) -> Option<u32> {
    is_dense_row(text).then(|| {
        if has_token(text, "dense-header") {
            DENSE_HEADER_HEIGHT
        } else {
            DENSE_ROW_HEIGHT
        }
    })
}

pub(super) fn is_resource_columns(text: Option<&str>) -> bool {
    has_token(text, "resource-columns")
}

pub(super) fn is_dense_table(text: Option<&str>) -> bool {
    has_token(text, "dense-table")
}

pub(super) fn is_sidebar_panel(text: Option<&str>) -> bool {
    has_token(text, "sidebar")
}

pub(super) fn is_file_detail_row(text: Option<&str>) -> bool {
    has_token(text, "file-detail-row")
}

pub(super) fn is_alternate_row(text: Option<&str>) -> bool {
    has_token(text, "alternate")
}

pub(super) fn is_files_shell(text: Option<&str>) -> bool {
    has_token(text, "files-shell")
}

pub(super) fn is_simplified_files_shell(text: Option<&str>) -> bool {
    is_files_shell(text) && has_token(text, "simplified")
}

pub(super) fn is_files_content(text: Option<&str>) -> bool {
    has_token(text, "files-content")
}

pub(super) fn is_files_status(text: Option<&str>) -> bool {
    has_token(text, "files-status")
}

pub(super) fn is_redmond98_files_toolbar(text: Option<&str>) -> bool {
    has_token(text, "files-toolbar") && has_token(text, "redmond98")
}

pub(super) fn is_redmond98_folder_tree(text: Option<&str>) -> bool {
    has_token(text, "redmond98-folder-tree")
}

pub(super) fn is_redmond98_webview(text: Option<&str>) -> bool {
    has_token(text, "redmond98-webview") && !has_token(text, "rule")
}

pub(super) fn is_redmond98_webview_rule(text: Option<&str>) -> bool {
    has_token(text, "redmond98-webview-rule")
}

pub(super) fn is_redmond98_details_view(text: Option<&str>) -> bool {
    has_token(text, "redmond98-details-view")
}

/// Marks a single-row form control that should use the preferred 40 px target
/// instead of the compact 24 px desktop minimum.
pub(super) fn is_form_control_row(text: Option<&str>) -> bool {
    has_token(text, "form-control")
}

pub(super) fn compact_split_threshold(text: Option<&str>, fallback: u32) -> u32 {
    text.and_then(|text| {
        text.split(':').find_map(|part| {
            part.strip_prefix("compact-at=")?
                .parse::<u32>()
                .ok()
                .map(|value| value.clamp(320, 1_200))
        })
    })
    .unwrap_or(fallback)
}

pub(super) fn aligns_trailing_action(text: Option<&str>) -> bool {
    has_token(text, TRAILING_ACTION.trim_start_matches("panel:"))
}

pub(super) fn is_feature_card_content(text: Option<&str>) -> bool {
    text == Some("panel:feature-card-content")
}

pub(super) fn is_feature_card_action(text: Option<&str>) -> bool {
    text == Some("panel:feature-card-action")
}

pub(super) fn has_spaced_separator(text: Option<&str>) -> bool {
    has_token(text, "spaced-separator")
}

pub(super) fn calendar_day_band(text: Option<&str>) -> Option<bool> {
    match text?
        .strip_prefix(CALENDAR_DAY_BAND_PREFIX)?
        .split(':')
        .next()?
    {
        "even" => Some(false),
        "odd" => Some(true),
        _ => None,
    }
}

pub(super) fn calendar_workday(text: Option<&str>) -> bool {
    text.is_some_and(|text| {
        text.starts_with(CALENDAR_DAY_BAND_PREFIX)
            && text.split(':').skip(1).any(|token| token == "workday")
    })
}

pub(super) fn action_width_hint<'a>(text: Option<&'a str>, key: &str) -> Option<&'a str> {
    text?.split(':').find_map(|part| part.strip_prefix(key))
}

pub(super) fn control_for(text: Option<&str>) -> Option<u64> {
    text?.split(':').find_map(|part| {
        part.strip_prefix("control-for=")?
            .parse::<u64>()
            .ok()
            .filter(|value| *value != 0)
    })
}

pub(super) fn preferred_line_height(widget: &Widget, theme: &Theme) -> u32 {
    let size = preferred_text_size(widget, theme);
    let percent = if widget.kind == rice_ui::WidgetKind::Markdown {
        // Canvas Markdown uses a 1.35 line box and can promote headings up to
        // 1.28x the base size. Reserve the same worst-case height here so the
        // final line is never silently clipped by retained layout.
        175
    } else if widget.state.contains(WidgetState::PROMINENT) {
        106
    } else {
        u32::from(theme.typography.line_height_percent)
    };
    u32::from(size).saturating_mul(percent) / 100
}

pub(super) fn zoom_permille(text: Option<&str>) -> u32 {
    text.and_then(|value| {
        value
            .strip_prefix(ZOOM_PREFIX)
            .or_else(|| value.strip_prefix(FILE_GRID_PREFIX))
    })
    .and_then(|value| value.parse::<u32>().ok())
    .unwrap_or(1_000)
    .clamp(500, 5_000)
}

pub(super) fn is_file_grid(text: Option<&str>) -> bool {
    text.is_some_and(|value| value.starts_with(FILE_GRID_PREFIX))
}

pub(super) fn is_thumbnail_cell(text: Option<&str>) -> bool {
    text.is_some_and(|text| text == THUMBNAIL_CELL || text.starts_with("panel:thumbnail-cell:"))
}

pub(super) fn is_desktop_thumbnail_cell(text: Option<&str>) -> bool {
    text == Some(DESKTOP_THUMBNAIL_CELL)
}

pub(super) fn is_app_grid(text: Option<&str>) -> bool {
    text.is_some_and(|text| text == APP_GRID || text.starts_with("app-grid:"))
}

pub(super) fn is_store_grid(text: Option<&str>) -> bool {
    is_app_grid(text) && has_token(text, "store")
}

pub(super) fn app_grid_rows(text: Option<&str>) -> u32 {
    text.and_then(|text| text.strip_prefix("app-grid:"))
        .and_then(|rows| rows.split(':').next()?.parse().ok())
        .unwrap_or(4)
        .clamp(3, if is_store_grid(text) { 16 } else { 8 })
}

pub(super) fn viewport_height(text: Option<&str>) -> Option<u32> {
    let text = text?.strip_prefix("sections:").and_then(|value| value.split_once('|').map(|(_, rest)| rest)).or(text)?;
    let value = text
        .split_once('|')?
        .0
        .strip_prefix(VIEWPORT_PREFIX)?
        .parse::<u32>()
        .ok()?;
    Some(value.clamp(64, 800))
}

pub(super) fn scroll_value(text: Option<&str>) -> Option<&str> {
    text.map(|value| value.rsplit('|').next().unwrap_or(value))
}

pub(super) fn scroll_anchor(text: Option<&str>) -> Option<u64> {
    scroll_value(text)?
        .strip_prefix("anchor:")?
        .parse::<u64>()
        .ok()
        .filter(|id| *id != 0)
}

/// Returns the logical Y origin of a bounded window rendered inside a much
/// larger scrollable document.
///
/// A virtual scroll view carries `virtual-window:<pixels>|<position>/<extent>`.
/// The host uses the full logical extent for its scrollbar, but only lays out
/// the bounded child window at this origin. This keeps large logs and tables
/// out of a single RAPP text field without changing ordinary ScrollView
/// semantics.
pub(super) fn virtual_window_offset(text: Option<&str>) -> Option<u32> {
    text?
        .split_once('|')?
        .0
        .strip_prefix("virtual-window:")?
        .parse::<u32>()
        .ok()
}

pub(super) fn fixed_height(text: Option<&str>) -> Option<u32> {
    let text = text?;
    if let Some(value) = accessory_extent(Some(text), "accessory-height=") { return Some(value); }
    if let Some(height) = text.strip_prefix("panel:document-height:") {
        return height.parse::<u32>().ok().map(|height| height.clamp(40, 100_000));
    }
    if text == "panel:welcome-hero" {
        return Some(560);
    }
    if text == "panel:welcome-journey" {
        return Some(250);
    }
    if text
        .split(':')
        .any(|token| token == "redmond98-calculator-display")
    {
        return text
            .strip_prefix(FIXED_PANEL_HEIGHT_PREFIX)?
            .split(':')
            .next()?
            .parse::<u32>()
            .ok()
            .map(|value| value.clamp(24, 800));
    }
    text.strip_prefix(FIXED_HEIGHT_PREFIX)
        .or_else(|| text.strip_prefix(FIXED_PANEL_HEIGHT_PREFIX))?
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .map(|value| value.clamp(64, 4096))
}

pub(super) fn fixed_width(text: Option<&str>) -> Option<u32> {
    if let Some(value) = accessory_extent(text, "accessory-width=") { return Some(value); }
    text?.split(':').find_map(|part| {
        part.strip_prefix("width=")?
            .parse::<u32>()
            .ok()
            // Explicit compact accessories (spin buttons, disclosure slots,
            // icon actions) are routinely narrower than a text field.  A
            // 120 px floor made those declarations lie and produced giant
            // controls even though the application requested 28–40 px.
            .map(|width| width.clamp(24, 4096))
    })
}

/// Thin native accessories, such as scroll tracks, without changing ordinary
/// panel minimum sizes relied on by existing applications.
fn accessory_extent(text: Option<&str>, prefix: &str) -> Option<u32> {
    text?.split(':').find_map(|part| part.strip_prefix(prefix)?.parse::<u32>().ok()
        .filter(|value| (1..=64).contains(value)))
}

pub(super) fn welcome_hero_height(text: Option<&str>, width: u32) -> Option<u32> {
    (text == Some("panel:welcome-hero")).then(|| {
        u32::try_from(u64::from(width).saturating_mul(944) / 1_776)
            .unwrap_or(u32::MAX)
            .clamp(480, 680)
    })
}

pub(super) fn preferred_text_size(widget: &Widget, theme: &Theme) -> u16 {
    if widget.kind == WidgetKind::TextEditor
        && let Some(font_size) = widget
            .text
            .as_deref()
            .and_then(rice_ui::EditorText::parse)
            .and_then(|editor| editor.font_size)
    {
        font_size
    } else if widget.state.contains(WidgetState::PROMINENT) {
        theme.typography.display_px.saturating_mul(2)
    } else if widget.state.contains(WidgetState::BRAND_TYPE)
        && widget.state.contains(WidgetState::DISPLAY)
    {
        theme
            .typography
            .display_px
            .saturating_add(theme.typography.title_px.saturating_mul(2))
    } else if widget.state.contains(WidgetState::BRAND_TYPE)
        && widget.state.contains(WidgetState::EMPHASIZED)
        && widget.state.contains(WidgetState::MUTED)
    {
        theme.typography.body_px.saturating_add(4)
    } else if widget.state.contains(WidgetState::BRAND_TYPE)
        && widget.state.contains(WidgetState::EMPHASIZED)
    {
        theme.typography.title_px.saturating_add(4)
    } else if widget.state.contains(WidgetState::BRAND_TYPE) {
        theme.typography.body_px.saturating_add(5)
    } else if widget.state.contains(WidgetState::DISPLAY) {
        theme.typography.display_px
    } else if widget.state.contains(WidgetState::EMPHASIZED) {
        theme.typography.title_px
    } else {
        theme.typography.body_px
    }
}

fn parse_clamp(text: &str) -> Option<u32> {
    text.strip_prefix(CLAMP_PREFIX)?
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .map(|value| value.clamp(MIN_CLAMP, MAX_CLAMP))
}

pub(super) fn has_token(text: Option<&str>, token: &str) -> bool {
    text.is_some_and(|text| text.split(':').any(|part| part == token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_roomy_content_and_preserves_compact_content() {
        let roomy = Rect {
            origin: Point { x: 4, y: 8 },
            size: Size {
                width: 992,
                height: 600,
            },
        };
        assert_eq!(
            clamped_content(Some("panel:clamp:520"), roomy),
            Rect {
                origin: Point { x: 240, y: 8 },
                size: Size {
                    width: 520,
                    height: 600
                }
            }
        );
        let compact = Rect {
            origin: Point { x: 4, y: 8 },
            size: Size {
                width: 312,
                height: 220,
            },
        };
        assert_eq!(clamped_content(Some("panel:clamp:520"), compact), compact);
    }

    #[test]
    fn distribute_is_an_exact_bounded_token() {
        assert!(distributes_children(Some("panel:distribute")));
        assert!(distributes_children(Some("panel:navigation-row")));
        assert!(!distributes_children(Some("panel:distribute:999")));
        assert!(!distributes_children(None));
        assert!(equal_fixed(Some("panel:equal-fixed")));
        assert!(!equal_fixed(Some("panel:equal")));
        assert_eq!(zoom_permille(Some("zoom:1250")), 1_250);
        assert_eq!(zoom_permille(Some("zoom:9")), 500);
        assert_eq!(zoom_permille(Some("file-grid:5000")), 5_000);
        assert_eq!(zoom_permille(Some("file-grid:9000")), 5_000);
        assert_eq!(zoom_permille(Some("zoom:nope")), 1_000);
        assert!(is_file_grid(Some("file-grid:2400")));
        assert!(is_thumbnail_cell(Some("panel:thumbnail-cell")));
        assert!(centers_vertical(Some("panel:clamp:620:center-content")));
        assert!(!centers_vertical(Some("panel:clamp:620")));
        assert!(is_settings_page(Some(
            "panel:clamp:780:settings-page:section-stack"
        )));
        assert!(is_section_stack(Some(
            "panel:clamp:780:settings-page:section-stack"
        )));
        assert!(is_settings_card(Some("panel:settings-card")));
        assert!(is_settings_shell(Some("panel:clamp:900:settings-shell")));
        assert_eq!(child_gap(Some("panel:settings-card"), 4), 4);
        assert_eq!(child_gap(Some("panel:section-stack"), 4), 8);
        assert!(is_window_control(Some("panel:window-control")));
        assert!(is_bookmark_bar(Some("panel:compact:bookmark-bar")));
        assert_eq!(
            compact_split_threshold(Some("split:260:compact-at=700"), 400),
            700
        );
        assert_eq!(compact_split_threshold(Some("split:260"), 400), 400);
        assert!(is_desktop_thumbnail_cell(Some("desktop:thumbnail-cell")));
        assert!(is_app_grid(Some("app-grid")));
        assert!(is_app_grid(Some("app-grid:5")));
        assert_eq!(app_grid_rows(Some("app-grid")), 4);
        assert_eq!(app_grid_rows(Some("app-grid:5")), 5);
        assert_eq!(app_grid_rows(Some("app-grid:10:store")), 10);
        assert_eq!(app_grid_rows(Some("app-grid:10")), 8);
        assert_eq!(
            control_for(Some("split:660:min-width=240:control-for=204")),
            Some(204)
        );
        assert_eq!(control_for(Some("control-for=0")), None);
        assert_eq!(viewport_height(Some("viewport:180|25/900")), Some(180));
        assert_eq!(scroll_value(Some("viewport:180|25/900")), Some("25/900"));
        assert_eq!(scroll_anchor(Some("anchor:3000")), Some(3_000));
        assert_eq!(scroll_anchor(Some("25/900")), None);
        assert_eq!(fixed_height(Some("panel:height:144")), Some(144));
        assert_eq!(fixed_height(Some("split:150:filter-strip")), None);
        assert!(is_filter_strip(Some("split:150:filter-strip")));
        assert!(is_dense_row(Some("split:390:compact:dense")));
        assert_eq!(dense_row_height(Some("split:390:compact:dense")), Some(36));
        assert_eq!(
            dense_row_height(Some("split:390:compact:dense:dense-header")),
            Some(20)
        );
        assert!(is_resource_columns(Some("split:390:resource-columns")));
        assert_eq!(child_gap(Some("panel:resource-timeline"), 8), 0);
        assert!(is_dense_table(Some("panel:fixed:width=720:dense-table")));
        assert!(is_sidebar_panel(Some("panel:sidebar")));
        assert!(is_file_detail_row(Some(
            "split:600:control-for=1000:file-detail-row:alternate"
        )));
        assert!(is_alternate_row(Some(
            "split:600:control-for=1000:file-detail-row:alternate"
        )));
        assert!(is_files_shell(Some("panel:files-shell")));
        assert!(is_simplified_files_shell(Some(
            "panel:files-shell:simplified"
        )));
        assert!(!is_simplified_files_shell(Some("panel:files-shell")));
        assert!(is_files_content(Some("split:220:files-content")));
        assert!(is_files_status(Some("panel:status-bar:files-status")));
        assert!(is_redmond98_files_toolbar(Some(
            "panel:files-toolbar:redmond98"
        )));
        assert!(is_redmond98_folder_tree(Some(
            "panel:sidebar:redmond98-folder-tree"
        )));
        assert!(is_redmond98_webview(Some("panel:redmond98-webview")));
        assert!(is_redmond98_webview_rule(Some(
            "panel:redmond98-webview-rule"
        )));
        assert_eq!(child_gap(Some("panel:fixed:width=720:dense-table"), 8), 0);
        assert!(is_store_header(Some("panel:store-header:intrinsic-flow")));
        assert_eq!(fixed_height(Some("panel:fixed-height:144")), Some(144));
        assert_eq!(
            fixed_height(Some("panel:fixed-height:520:settings-page:section-stack")),
            Some(520)
        );
        assert_eq!(fixed_height(Some("panel:height:240:compact")), Some(240));
        assert_eq!(
            fixed_height(Some("panel:fixed-height:34:redmond98-calculator-display")),
            Some(34)
        );
        assert_eq!(
            calendar_day_band(Some("panel:calendar-day-band-even")),
            Some(false)
        );
        assert_eq!(
            calendar_day_band(Some("panel:calendar-day-band-odd")),
            Some(true)
        );
        assert_eq!(
            calendar_day_band(Some("panel:calendar-day-band-even:workday")),
            Some(false)
        );
        assert!(calendar_workday(Some(
            "panel:calendar-day-band-even:workday"
        )));
        assert!(!calendar_workday(Some("panel:calendar-day-band-even")));
        assert_eq!(
            calendar_day_band(Some("panel:calendar-day-band-nope")),
            None
        );
    }

    #[test]
    fn virtual_window_metadata_preserves_the_scroll_fraction() {
        let metadata = Some("virtual-window:4096|4096/240000");
        assert_eq!(virtual_window_offset(metadata), Some(4_096));
        assert_eq!(scroll_value(metadata), Some("4096/240000"));
        assert_eq!(virtual_window_offset(Some("4096/240000")), None);
        assert_eq!(virtual_window_offset(Some("virtual-window:nope|0/1")), None);
    }
}

/// A bounded application-owned arrangement surface, using ordinary themed
/// child widgets. Coordinates are permille of its content rectangle.
/// Transparent positioned children that do not participate in parent sizing.
/// Unlike a document canvas this follows its Stack bounds without aspect fitting.
pub(super) fn positioned_overlay(text: Option<&str>) -> bool {
    matches!(text, Some("panel:overlay") | Some("panel:overlay:flush"))
}
pub(super) fn positioned_canvas_height(text: Option<&str>) -> Option<u32> {
    text?.strip_prefix("positioned-canvas:")?.parse::<u32>().ok().filter(|height| (120..=600).contains(height))
}
pub(super) fn positioned_item(text: Option<&str>) -> Option<[u32;4]> {
    let values: Vec<u32> = text?.strip_prefix("positioned-item:")?.split(':')
        .map(str::parse).collect::<Result<_,_>>().ok()?;
    let values: [u32;4] = values.try_into().ok()?;
    (values.iter().all(|value| *value<=1000) && values[2]>0 && values[3]>0 &&
        values[0]+values[2]<=1000 && values[1]+values[3]<=1000).then_some(values)
}

/// A bounded popup positioned relative to its ordinary parent widget.
pub(super) fn anchored_popup(text: Option<&str>) -> Option<[u32; 4]> {
    let values: Vec<u32> = text?.strip_prefix("panel:anchored-popup:")?.split(':')
        .map(str::parse).collect::<Result<_, _>>().ok()?;
    let values: [u32; 4] = values.try_into().ok()?;
    (values[0] <= 65536 && values[1] <= 65536 && (240..=640).contains(&values[2])
        && (160..=800).contains(&values[3])).then_some(values)
}

/// Plain editors own their bounded contents and scroll offset.
pub(super) fn is_editor_viewport(text: Option<&str>) -> bool {
    text.is_some_and(|text| text.starts_with("editor-viewport|"))
}
