//! Visible, cancellable progress for multi-item VFS mutations.

use rice_ui::{IconId, Patch, WidgetKind};

use super::files_view::{ROOT, create, tail_elided_path};

pub(super) const CANCEL: u64 = 300_100;
const PANEL: u64 = 300_101;
const LABEL: u64 = 300_102;
const BAR: u64 = 300_103;
const SUMMARY_ROW: u64 = 300_104;
const CANCEL_GROUP: u64 = 300_105;

pub(super) struct ProgressView<'a> {
    pub label: &'a str,
    pub completed: u64,
    pub total: u64,
    pub current: &'a str,
    pub cancel_requested: bool,
    pub byte_progress: Option<(u64, u64)>,
}

pub(super) fn patches(patches: &mut Vec<Patch>, progress: &ProgressView<'_>) {
    create(
        patches,
        PANEL,
        Some(ROOT),
        WidgetKind::Container,
        Some("panel:fixed"),
        None,
    );
    let current = tail_elided_path(progress.current, 48);
    let operation = rice_i18n::t(progress.label);
    let completed = progress.completed.to_string();
    let total = progress.total.to_string();
    let label = if progress.cancel_requested {
        rice_i18n::format("Cancelling {operation} after the current item finishes…", &[("operation", operation)])
    } else {
        match progress.byte_progress {
            Some((bytes, total_bytes)) => rice_i18n::format(
                "{operation}: {completed}/{total} · {bytes} / {total_bytes} · {percent}% · {path}",
                &[("operation", operation), ("completed", &completed), ("total", &total),
                  ("bytes", &size_label(bytes)), ("total_bytes", &size_label(total_bytes)),
                  ("percent", &percent_label(bytes, total_bytes)), ("path", &current)],
            ),
            None => rice_i18n::format("{operation}: {completed}/{total} · {path}",
                &[("operation", operation), ("completed", &completed), ("total", &total), ("path", &current)]),
        }
    };
    create(
        patches,
        SUMMARY_ROW,
        Some(PANEL),
        WidgetKind::Row,
        Some("panel:distribute"),
        None,
    );
    create(
        patches,
        LABEL,
        Some(SUMMARY_ROW),
        WidgetKind::Label,
        Some(&label),
        Some(IconId::Information),
    );
    let bar = match progress.byte_progress {
        Some((completed, total)) if total > 0 => format!("seek:{completed}/{total}"),
        _ if progress.total > 0 => format!("seek:{}/{}", progress.completed, progress.total),
        _ => String::from("seek:0/1"),
    };
    create(
        patches,
        BAR,
        Some(PANEL),
        WidgetKind::ProgressBar,
        Some(&bar),
        None,
    );
    if !progress.cancel_requested {
        create(
            patches,
            CANCEL_GROUP,
            Some(SUMMARY_ROW),
            WidgetKind::Row,
            Some("panel:equal-fixed"),
            None,
        );
        create(
            patches,
            CANCEL,
            Some(CANCEL_GROUP),
            WidgetKind::Button,
            Some(rice_i18n::t("Cancel")),
            Some(IconId::Close),
        );
    }
}

fn size_label(bytes: u64) -> String {
    super::files_details::total_size_label(bytes)
}

fn percent_label(completed: u64, total: u64) -> String {
    if total == 0 {
        return String::from("0");
    }
    let hundredths = completed.saturating_mul(10_000) / total;
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_retranslates_operation_without_changing_paths_or_wire_values() {
        let mut progress = ProgressView {
            label: "Copy", completed: 1, total: 3,
            current: "/root/{operation}.txt", cancel_requested: false,
            byte_progress: Some((512, 1024)),
        };
        for (locale, operation) in [(1, "Copiar"), (2, "复制")] {
            rice_i18n::with_locale(locale, || {
                let mut output = Vec::new();
                patches(&mut output, &progress);
                let text = |id| output.iter().find_map(|patch| match patch {
                    Patch::Create { widget, .. } if widget.id.get() == id => widget.text.as_deref(),
                    _ => None,
                });
                let label = text(LABEL).unwrap();
                assert!(label.starts_with(operation), "{label}");
                assert!(label.contains("/root/{operation}.txt"), "{label}");
                assert!(label.contains("50.00%"), "{label}");
                assert_eq!(text(BAR), Some("seek:512/1024"));
            });
        }
        progress.cancel_requested = true;
        rice_i18n::with_locale(2, || {
            let mut output = Vec::new();
            patches(&mut output, &progress);
            assert!(output.iter().any(|patch| matches!(patch,
                Patch::Create { widget, .. } if widget.text.as_deref() == Some("当前项目完成后取消复制…"))));
            assert!(!output.iter().any(|patch| matches!(patch,
                Patch::Create { widget, .. } if widget.id.get() == CANCEL)));
        });
    }
}
