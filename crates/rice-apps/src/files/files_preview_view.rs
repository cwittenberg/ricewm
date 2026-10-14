//! Sushi-like preview pane backed by the trusted host media cache.

use super::{
    files_preview::PreviewState,
    files_view::{MAIN_SPLIT, PREVIEW_PANEL, create, create_with_state},
};
use rice_native_app_protocol::MediaKind;
use rice_ui::{EditorWrap, IconId, Patch, ValueChange, WidgetKind, WidgetState, editor_text};

const PREVIEW_TITLE: u64 = 210_001;
pub(super) const PREVIEW_MEDIA: u64 = 210_002;
const PREVIEW_DETAIL: u64 = 210_003;
const PREVIEW_FEEDBACK: u64 = 210_004;
pub(super) const PREVIEW_PLAY: u64 = 210_005;
pub(super) const PREVIEW_PROGRESS: u64 = 210_006;
const PREVIEW_CONTROLS: u64 = 210_007;
const PREVIEW_PLAY_TOOLTIP: u64 = 210_008;
pub(super) const PREVIEW_FULLSCREEN: u64 = 210_009;
pub(super) const PREVIEW_OPEN_PLAYER: u64 = 210_010;
const PREVIEW_FULLSCREEN_TOOLTIP: u64 = 210_011;
const PREVIEW_OPEN_TOOLTIP: u64 = 210_012;

pub(super) fn patches(patches: &mut Vec<Patch>, state: &PreviewState) {
    patches_in(patches, state, MAIN_SPLIT, false);
}

pub(super) fn fullscreen_patches(patches: &mut Vec<Patch>, state: &PreviewState) {
    patches_in(patches, state, super::files_view::ROOT, true);
}

fn patches_in(patches: &mut Vec<Patch>, state: &PreviewState, parent: u64, fullscreen: bool) {
    create(
        patches,
        PREVIEW_PANEL,
        Some(parent),
        WidgetKind::Column,
        None,
        None,
    );
    match state {
        PreviewState::Idle => create_with_state(
            patches,
            PREVIEW_FEEDBACK,
            Some(PREVIEW_PANEL),
            WidgetKind::Image,
            None,
            Some(IconId::ImageViewer),
            WidgetState::MUTED,
        ),
        PreviewState::Loading { name, .. } | PreviewState::VideoLoading { name, .. } => feedback(
            patches,
            name,
            rice_i18n::t("Loading preview…"),
            IconId::Refresh,
            WidgetKind::Spinner,
        ),
        PreviewState::TextLoading { name, .. } => feedback(
            patches,
            name,
            rice_i18n::t("Loading text preview…"),
            IconId::Document,
            WidgetKind::Spinner,
        ),
        PreviewState::TextReady {
            name,
            path,
            mime_type,
            text,
        } => {
            create(
                patches,
                PREVIEW_TITLE,
                Some(PREVIEW_PANEL),
                WidgetKind::Label,
                Some(name),
                Some(IconId::Document),
            );
            let language = text_language(path, mime_type);
            create(
                patches,
                PREVIEW_MEDIA,
                Some(PREVIEW_PANEL),
                WidgetKind::TextEditor,
                Some(&editor_text(language, EditorWrap::Word, true, text)),
                None,
            );
            create(
                patches,
                PREVIEW_DETAIL,
                Some(PREVIEW_PANEL),
                WidgetKind::Label,
                Some(rice_i18n::t("Embedded read-only RicePaper preview")),
                None,
            );
        }
        PreviewState::OwnedError { name, message } => {
            feedback(patches, name, rice_i18n::t(message), IconId::Warning, WidgetKind::Label);
        }
        PreviewState::Error { name, message } => {
            feedback(patches, name, message, IconId::Warning, WidgetKind::Label);
        }
        PreviewState::Ready {
            request,
            name,
            metadata,
            ..
        } => {
            create(
                patches,
                PREVIEW_TITLE,
                Some(PREVIEW_PANEL),
                WidgetKind::Label,
                Some(name),
                media_icon(metadata.kind),
            );
            create(
                patches,
                PREVIEW_MEDIA,
                Some(PREVIEW_PANEL),
                WidgetKind::Image,
                Some(&format!("media-request:{}", request.get())),
                None,
            );
            create(
                patches,
                PREVIEW_DETAIL,
                Some(PREVIEW_PANEL),
                WidgetKind::Label,
                Some(&metadata_label(metadata)),
                None,
            );
        }
        PreviewState::Video {
            session,
            name,
            duration_millis,
            position_millis,
            playing,
            ..
        } => {
            create(
                patches,
                PREVIEW_TITLE,
                Some(PREVIEW_PANEL),
                WidgetKind::Label,
                Some(name),
                Some(IconId::Application),
            );
            create(
                patches,
                PREVIEW_MEDIA,
                Some(PREVIEW_PANEL),
                WidgetKind::Image,
                Some(&format!("media-request:{}", session)),
                None,
            );
            create(
                patches,
                PREVIEW_CONTROLS,
                Some(PREVIEW_PANEL),
                WidgetKind::ToolBar,
                None,
                None,
            );
            create_with_state(
                patches,
                PREVIEW_PLAY,
                Some(PREVIEW_CONTROLS),
                WidgetKind::Button,
                None,
                Some(if *playing {
                    IconId::MediaPause
                } else {
                    IconId::MediaPlay
                }),
                WidgetState::ENABLED,
            );
            create(
                patches,
                PREVIEW_PLAY_TOOLTIP,
                Some(PREVIEW_PLAY),
                WidgetKind::Tooltip,
                Some(rice_i18n::t(if *playing { "Pause" } else { "Play" })),
                None,
            );
            create(
                patches,
                PREVIEW_PROGRESS,
                Some(PREVIEW_CONTROLS),
                WidgetKind::ProgressBar,
                Some(&format!(
                    "seek:{position_millis}/{}",
                    (*duration_millis).max(1)
                )),
                None,
            );
            for (id, tooltip_id, icon, tooltip) in [
                (
                    PREVIEW_FULLSCREEN,
                    PREVIEW_FULLSCREEN_TOOLTIP,
                    if fullscreen {
                        IconId::Restore
                    } else {
                        IconId::Fullscreen
                    },
                    if fullscreen {
                        rice_i18n::t("Exit fullscreen (Esc)")
                    } else {
                        rice_i18n::t("Fullscreen preview (double-click)")
                    },
                ),
                (
                    PREVIEW_OPEN_PLAYER,
                    PREVIEW_OPEN_TOOLTIP,
                    IconId::Video,
                    rice_i18n::t("Open in RicePlayer (Enter)"),
                ),
            ] {
                create_with_state(
                    patches,
                    id,
                    Some(PREVIEW_CONTROLS),
                    WidgetKind::Button,
                    None,
                    Some(icon),
                    WidgetState::ENABLED,
                );
                create(
                    patches,
                    tooltip_id,
                    Some(id),
                    WidgetKind::Tooltip,
                    Some(tooltip),
                    None,
                );
            }
        }
    }
}

fn text_language(path: &str, mime_type: &str) -> &'static str {
    let extension = path.rsplit_once('.').map(|(_, extension)| extension);
    match extension.unwrap_or_default().to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "py" => "python",
        "c" | "h" | "cc" | "cpp" => "c",
        "js" | "mjs" | "jsx" | "ts" | "tsx" => "javascript",
        "sh" | "bash" | "zsh" => "shell",
        "html" | "xml" => "html",
        "css" => "css",
        "json" => "json",
        "md" => "markdown",
        _ if mime_type == "text/csv" => "csv",
        _ => "plain",
    }
}

fn feedback(patches: &mut Vec<Patch>, title: &str, detail: &str, icon: IconId, kind: WidgetKind) {
    create(
        patches,
        PREVIEW_TITLE,
        Some(PREVIEW_PANEL),
        WidgetKind::Label,
        Some(title),
        Some(icon),
    );
    create(
        patches,
        PREVIEW_FEEDBACK,
        Some(PREVIEW_PANEL),
        kind,
        Some(detail),
        None,
    );
}

fn metadata_label(metadata: &rice_native_app_protocol::MediaMetadata) -> String {
    let kind = match metadata.kind {
        MediaKind::Image => rice_i18n::t("Image"),
        MediaKind::Video => rice_i18n::t("Movie preview"),
    };
    let width = metadata.width.to_string();
    let height = metadata.height.to_string();
    match metadata.duration_millis {
        Some(value) => rice_i18n::format("{kind} • {width} × {height} • {seconds} s", &[("kind", kind), ("width", &width), ("height", &height), ("seconds", &format!("{:.1}", value as f64 / 1_000.0))]),
        None => rice_i18n::format("{kind} • {width} × {height}", &[("kind", kind), ("width", &width), ("height", &height)]),
    }
}

const fn media_icon(kind: MediaKind) -> Option<IconId> {
    Some(match kind {
        MediaKind::Image => IconId::DocumentOpen,
        MediaKind::Video => IconId::Application,
    })
}

pub(super) fn video_control_updates(state: &PreviewState) -> Vec<Patch> {
    use super::files_view::widget_id;
    let PreviewState::Video {
        position_millis,
        duration_millis,
        playing,
        ..
    } = state
    else {
        return Vec::new();
    };
    vec![
        Patch::Update {
            id: widget_id(PREVIEW_PROGRESS),
            state: None,
            text: ValueChange::Set(format!(
                "seek:{position_millis}/{}",
                (*duration_millis).max(1)
            )),
            icon: ValueChange::Keep,
        },
        Patch::Update {
            id: widget_id(PREVIEW_PLAY),
            state: None,
            text: ValueChange::Keep,
            icon: ValueChange::Set(if *playing {
                IconId::MediaPause
            } else {
                IconId::MediaPlay
            }),
        },
        Patch::Update {
            id: widget_id(PREVIEW_PLAY_TOOLTIP),
            state: None,
            text: ValueChange::Set(rice_i18n::t(if *playing { "Pause" } else { "Play" }).into()),
            icon: ValueChange::Keep,
        },
    ]
}

#[cfg(test)]
mod translation_tests {
    use super::*;

    #[test]
    fn preview_tooltips_refresh_without_changing_media_protocol_or_user_text() {
        let state = PreviewState::Video {
            session: 42, name: "Play {seconds}.mp4".into(), path: "/root/Play {seconds}.mp4".into(),
            duration_millis: 8000, position_millis: 2000, playing: true,
        };
        for (locale, pause, fullscreen, open) in [(1, "Pausar", "Vista previa a pantalla completa (doble clic)", "Abrir en RicePlayer (Enter)"),
                                                 (2, "暂停", "全屏预览（双击）", "在 RicePlayer 中打开（Enter）")] {
            rice_i18n::with_locale(locale, || {
                let mut output = Vec::new(); patches(&mut output, &state);
                for text in [pause, fullscreen, open, "Play {seconds}.mp4", "media-request:42", "seek:2000/8000"] {
                    assert!(output.iter().any(|p| matches!(p, Patch::Create {widget, ..} if widget.text.as_deref() == Some(text))), "missing {text}");
                }
                assert!(video_control_updates(&state).iter().any(|p| matches!(p,
                    Patch::Update {id, text: ValueChange::Set(text), ..} if id.get() == PREVIEW_PLAY_TOOLTIP && text == pause)));
            });
        }
        rice_i18n::with_locale(2, || {
            let mut output = Vec::new();
            patches(&mut output, &PreviewState::OwnedError { name: "Movie could not be opened".into(), message: "Movie could not be opened" });
            assert!(output.iter().any(|p| matches!(p, Patch::Create {widget, ..} if widget.id.get() == PREVIEW_FEEDBACK && widget.text.as_deref() == Some("无法打开视频"))));
            output.clear();
            patches(&mut output, &PreviewState::Error { name: "file.mp4".into(), message: "Movie could not be opened".into() });
            assert!(output.iter().any(|p| matches!(p, Patch::Create {widget, ..} if widget.id.get() == PREVIEW_FEEDBACK && widget.text.as_deref() == Some("Movie could not be opened"))));
        });
    }
}
