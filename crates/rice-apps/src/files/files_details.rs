//! Human-readable dependency-free labels for the Files detail tree.

use rice_files::{EntryKind, EntryMetadata};

/// Localize only the desktop's recognized standard directories. Never translate
/// arbitrary basenames or mutate the path used by file operations.
pub(super) fn display_name(entry: &EntryMetadata) -> &str {
    if entry.kind != EntryKind::Directory {
        return &entry.name;
    }
    let label = match entry.path.as_str() {
        "/root/Desktop" => "Desktop",
        "/root/Documents" => "Documents",
        "/root/Downloads" => "Downloads",
        "/root/Pictures" => "Pictures",
        "/root/Music" => "Music",
        "/root/Videos" => "Videos",
        _ => return &entry.name,
    };
    rice_i18n::t(label)
}

pub(super) fn name_label(entry: &EntryMetadata, location: &str) -> String {
    let base_depth = location.split('/').filter(|part| !part.is_empty()).count();
    let parent_depth = entry.path.parent().components().count();
    let indent = "  ".repeat(parent_depth.saturating_sub(base_depth));
    format!("{indent}{}", display_name(entry))
}

pub(super) fn empty_directory_label(entry: &EntryMetadata, location: &str) -> String {
    let base_depth = location.split('/').filter(|part| !part.is_empty()).count();
    let depth = entry.path.components().count().saturating_sub(base_depth);
    format!("{}{}", "  ".repeat(depth), rice_i18n::t("This directory is empty"))
}

pub(super) fn size_label(entry: &EntryMetadata) -> String {
    if entry.kind == EntryKind::Directory {
        return String::from("—");
    }
    match entry.size {
        bytes @ 0..1_024 => format!("{bytes} B"),
        bytes @ 1_024..1_048_576 => scaled_size(bytes, 1_024, "KB"),
        bytes @ 1_048_576..1_073_741_824 => scaled_size(bytes, 1_048_576, "MB"),
        bytes => scaled_size(bytes, 1_073_741_824, "GB"),
    }
}

pub(super) fn type_label(entry: &EntryMetadata) -> String {
    match entry.kind {
        EntryKind::Directory => String::from(rice_i18n::t("File Folder")),
        EntryKind::Symlink => String::from(rice_i18n::t("Shortcut")),
        EntryKind::Other => String::from(rice_i18n::t("Object")),
        EntryKind::File => {
            let extension = entry
                .name
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_ascii_uppercase())
                .filter(|extension| !extension.is_empty());
            match extension.as_deref() {
                Some("BMP") => String::from(rice_i18n::t("Bitmap Image")),
                Some("CAB") => String::from(rice_i18n::t("Cabinet")),
                Some("JPG" | "JPEG") => String::from(rice_i18n::t("JPEG Image")),
                Some("PNG") => String::from(rice_i18n::t("PNG Image")),
                Some("GIF") => String::from(rice_i18n::t("GIF Image")),
                Some("TXT") => String::from(rice_i18n::t("Text Document")),
                Some(extension) => rice_i18n::format("files.type.extension", &[("extension", extension)]),
                None => String::from(rice_i18n::t("File")),
            }
        }
    }
}

pub(super) fn total_size_label(bytes: u64) -> String {
    match bytes {
        bytes @ 0..1_024 => format!("{bytes} B"),
        bytes @ 1_024..1_048_576 => scaled_size(bytes, 1_024, "KB"),
        bytes @ 1_048_576..1_073_741_824 => scaled_size(bytes, 1_048_576, "MB"),
        bytes @ 1_073_741_824..1_099_511_627_776 => scaled_size(bytes, 1_073_741_824, "GB"),
        bytes => scaled_size(bytes, 1_099_511_627_776, "TB"),
    }
}

fn scaled_size(bytes: u64, unit: u64, suffix: &str) -> String {
    let whole = bytes / unit;
    let tenth = bytes % unit * 10 / unit;
    format!("{whole}.{tenth} {suffix}")
}

pub(super) fn modified_label(unix_ms: i64) -> String {
    let days = unix_ms.div_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    rice_i18n::format("files.modified-date", &[("month", month_name(month)), ("day", &day.to_string()), ("year", &year.to_string())])
}

// Howard Hinnant's civil-from-days mapping, adapted to Unix epoch days.
fn civil_from_days(unix_days: i64) -> (i64, u8, u8) {
    let days = unix_days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (
        year,
        u8::try_from(month).unwrap_or(1),
        u8::try_from(day).unwrap_or(1),
    )
}

fn month_name(month: u8) -> &'static str {
    match month {
        1 => rice_i18n::t("files.month.1"),
        2 => rice_i18n::t("files.month.2"),
        3 => rice_i18n::t("files.month.3"),
        4 => rice_i18n::t("files.month.4"),
        5 => rice_i18n::t("ui.may"),
        6 => rice_i18n::t("files.month.6"),
        7 => rice_i18n::t("files.month.7"),
        8 => rice_i18n::t("files.month.8"),
        9 => rice_i18n::t("files.month.9"),
        10 => rice_i18n::t("files.month.10"),
        11 => rice_i18n::t("files.month.11"),
        12 => rice_i18n::t("files.month.12"),
        _ => "—",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rice_files::GuestPath;

    #[test]
    fn rows_use_human_sizes_dates_and_directory_placeholders() {
        let directory = EntryMetadata::new(
            GuestPath::absolute("/Documents").expect("path"),
            EntryKind::Directory,
            0,
            0,
        );
        assert_eq!(name_label(&directory, "/root"), "Documents");
        assert_eq!(size_label(&directory), "—");
        assert_eq!(modified_label(0), "Jan 1, 1970");

        let file = EntryMetadata::new(
            GuestPath::absolute("/photo.png").expect("path"),
            EntryKind::File,
            1_536,
            1_704_067_200_000,
        );
        assert_eq!(name_label(&file, "/root"), "photo.png");
        assert_eq!(size_label(&file), "1.5 KB");
        assert_eq!(modified_label(file.modified_unix_ms), "Jan 1, 2024");
    }
}

#[cfg(test)]
mod localized_folder_tests {
    use super::*;
    use rice_files::GuestPath;

    #[test]
    fn localized_details_preserve_extensions_paths_and_timestamps() {
        let file = EntryMetadata::new(GuestPath::absolute("/root/Save.xyz").unwrap(), EntryKind::File, 1536, 0);
        let directory = EntryMetadata::new(GuestPath::absolute("/root/Projects").unwrap(), EntryKind::Directory, 0, 0);
        for (locale, kind, date, empty) in [
            (0, "XYZ File", "Jan 1, 1970", "This directory is empty"),
            (1, "Archivo XYZ", "1 ene 1970", "Esta carpeta está vacía"),
            (2, "XYZ 文件", "1970年1月1日", "此文件夹为空"),
        ] {
            rice_i18n::with_locale(locale, || {
                assert_eq!(type_label(&file), kind);
                assert_eq!(modified_label(file.modified_unix_ms), date);
                assert_eq!(empty_directory_label(&directory, "/root").trim(), empty);
                assert_eq!(name_label(&file, "/root"), "Save.xyz");
                assert_eq!(file.path.as_str(), "/root/Save.xyz");
                assert_eq!(file.modified_unix_ms, 0);
            });
        }
    }

    #[test]
    fn standard_folder_labels_do_not_change_paths_or_user_names() {
        for (locale, translated) in [(1, "Documentos"), (2, "文档"), (0, "Documents")] {
            rice_i18n::with_locale(locale, || {
                let standard = EntryMetadata::new(
                    GuestPath::absolute("/root/Documents").unwrap(), EntryKind::Directory, 0, 0);
                assert_eq!(display_name(&standard), translated);
                assert_eq!(name_label(&standard, "/root"), translated);
                assert_eq!(standard.path.as_str(), "/root/Documents");
                assert_eq!(standard.name, "Documents");
                for (path, kind) in [
                    ("/root/projects/Documents", EntryKind::Directory),
                    ("/mnt/local/Documents", EntryKind::Directory),
                    ("/root/Documents", EntryKind::File),
                    ("/root/Documents", EntryKind::Symlink),
                ] {
                    let custom = EntryMetadata::new(GuestPath::absolute(path).unwrap(), kind, 0, 0);
                    assert_eq!(display_name(&custom), "Documents");
                }
            });
        }
    }
}
