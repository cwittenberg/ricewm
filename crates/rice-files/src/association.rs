//! Bounded first-party MIME associations and validated launch intents.

use crate::{EntryKind, EntryMetadata, GuestPath};
use core::fmt;

pub const MAX_ASSOCIATION_CANDIDATES: usize = 16;
const MAX_MEDIA_TYPE_BYTES: usize = 127;
pub const MAX_DESKTOP_ENTRY_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationAssociation {
    pub entrypoint: &'static str,
    pub name: &'static str,
    pub media_patterns: &'static [&'static str],
}

const RICE_PAPER: ApplicationAssociation = ApplicationAssociation {
    entrypoint: "rice-paper",
    name: "RicePaper",
    media_patterns: &[
        "text/*",
        "application/json",
        "application/toml",
        "application/xml",
        "application/javascript",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.oasis.opendocument.text",
    ],
};

const RICE_VIEW: ApplicationAssociation = ApplicationAssociation {
    entrypoint: "rice-view",
    name: "RiceView",
    media_patterns: &["image/*", "application/pdf"],
};

const RICE_PLAYER: ApplicationAssociation = ApplicationAssociation {
    entrypoint: "rice-player",
    name: "Rice Player",
    media_patterns: &["video/*"],
};

const APPLICATIONS: &[ApplicationAssociation] = &[RICE_PAPER, RICE_VIEW, RICE_PLAYER];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopLaunchIntent {
    pub entrypoint: &'static str,
    pub arguments: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DesktopExecutable {
    aliases: &'static [&'static str],
    entrypoint: &'static str,
}

const DESKTOP_EXECUTABLES: &[DesktopExecutable] = &[
    DesktopExecutable {
        aliases: &["control-gallery"],
        entrypoint: "control-gallery",
    },
    DesktopExecutable {
        aliases: &["leafpad", "rice-paper"],
        entrypoint: "rice-paper",
    },
    DesktopExecutable {
        aliases: &["pcmanfm", "rice-files"],
        entrypoint: "rice-files",
    },
    DesktopExecutable {
        aliases: &["riceview", "rice-view"],
        entrypoint: "rice-view",
    },
    DesktopExecutable {
        aliases: &["galculator", "rice-calc"],
        entrypoint: "rice-calc",
    },
    DesktopExecutable {
        aliases: &["rice-player"],
        entrypoint: "rice-player",
    },
    DesktopExecutable {
        aliases: &["bowl"],
        entrypoint: "bowl",
    },
    DesktopExecutable {
        aliases: &["rice-settings"],
        entrypoint: "rice-settings",
    },
    DesktopExecutable {
        aliases: &["rice-performance"],
        entrypoint: "rice-performance",
    },
    DesktopExecutable {
        aliases: &["rice-profiler"],
        entrypoint: "rice-profiler",
    },
    DesktopExecutable {
        aliases: &["rice-setup"],
        entrypoint: "rice-setup",
    },
    DesktopExecutable {
        aliases: &["rice-extensions"],
        entrypoint: "rice-extensions",
    },
    DesktopExecutable {
        aliases: &["rice-term", "rice-terminal"],
        entrypoint: "rice-term",
    },
    DesktopExecutable {
        aliases: &["rice-day"],
        entrypoint: "rice-day",
    },
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AssociationRegistry;

impl AssociationRegistry {
    #[must_use]
    pub const fn first_party() -> Self {
        Self
    }

    #[must_use]
    pub fn candidates(self, entry: &EntryMetadata) -> Vec<ApplicationAssociation> {
        if entry.kind != EntryKind::File {
            return Vec::new();
        }
        let media_type = infer_media_type(entry);
        APPLICATIONS
            .iter()
            .copied()
            .filter(|application| {
                application
                    .media_patterns
                    .iter()
                    .any(|pattern| media_matches(pattern, media_type))
            })
            .take(MAX_ASSOCIATION_CANDIDATES)
            .collect()
    }

    /// Returns every registered application for an explicit Open With choice.
    #[must_use]
    pub fn installed(self) -> Vec<ApplicationAssociation> {
        APPLICATIONS
            .iter()
            .copied()
            .take(MAX_ASSOCIATION_CANDIDATES)
            .collect()
    }

    #[must_use]
    pub fn default_for(self, entry: &EntryMetadata) -> Option<ApplicationAssociation> {
        self.candidates(entry).into_iter().next()
    }

    /// Produces a launch only for a registered candidate and an absolute guest path.
    ///
    /// # Errors
    ///
    /// Returns an error when the file type is unsupported or `chosen_entrypoint`
    /// is not one of the registry's matching first-party applications.
    pub fn launch(
        self,
        entry: &EntryMetadata,
        chosen_entrypoint: Option<&str>,
    ) -> Result<LaunchIntent, AssociationError> {
        let candidates = self.candidates(entry);
        let application = chosen_entrypoint.map_or_else(
            || candidates.first().copied(),
            |chosen| {
                APPLICATIONS
                    .iter()
                    .find(|candidate| candidate.entrypoint == chosen)
                    .copied()
            },
        );
        let application = application.ok_or_else(|| {
            if chosen_entrypoint.is_some() {
                AssociationError::UnregisteredApplication
            } else {
                AssociationError::UnsupportedMediaType
            }
        })?;
        Ok(LaunchIntent {
            entrypoint: application.entrypoint,
            path: entry.path.clone(),
            media_type: infer_media_type(entry).to_owned(),
        })
    }

    /// Parses a desktop launcher without invoking a shell.
    ///
    /// Only registered first-party aliases are accepted. Field codes expand
    /// from explicit guest paths; all other arguments remain literal.
    ///
    /// # Errors
    ///
    /// Returns [`DesktopEntryError`] when the entry is malformed, hidden,
    /// oversized, names an unregistered executable, or uses unsafe field codes.
    pub fn launch_desktop_entry(
        self,
        bytes: &[u8],
        paths: &[GuestPath],
    ) -> Result<DesktopLaunchIntent, DesktopEntryError> {
        if bytes.len() > MAX_DESKTOP_ENTRY_BYTES {
            return Err(DesktopEntryError::TooLarge);
        }
        let text = core::str::from_utf8(bytes).map_err(|_| DesktopEntryError::InvalidUtf8)?;
        let mut in_entry = false;
        let mut entry_type = None;
        let mut hidden = false;
        let mut exec = None;
        let mut url = None;
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                in_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_entry {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "Type" if entry_type.is_none() => entry_type = Some(value.trim()),
                "Hidden" | "NoDisplay" if value.trim().eq_ignore_ascii_case("true") => {
                    hidden = true;
                }
                "Exec" if exec.is_none() => exec = Some(value.trim()),
                "URL" if url.is_none() => url = Some(value.trim()),
                _ => {}
            }
        }
        if hidden {
            return Err(DesktopEntryError::Hidden);
        }
        if entry_type == Some("Link") {
            let path = local_file_url(url.ok_or(DesktopEntryError::MissingUrl)?)?;
            return Ok(DesktopLaunchIntent {
                entrypoint: "rice-files",
                arguments: vec![path.as_str().to_owned()],
            });
        }
        if entry_type != Some("Application") {
            return Err(DesktopEntryError::NotApplication);
        }
        let tokens = tokenize_exec(exec.ok_or(DesktopEntryError::MissingExec)?)?;
        let executable = tokens.first().ok_or(DesktopEntryError::MissingExec)?;
        let basename = executable.rsplit('/').next().unwrap_or(executable);
        let registered = DESKTOP_EXECUTABLES
            .iter()
            .find(|candidate| candidate.aliases.contains(&basename))
            .ok_or(DesktopEntryError::UnregisteredExecutable)?;
        if registered.entrypoint == "rice-files" {
            let arguments = tokens
                .iter()
                .skip(1)
                .rev()
                .find_map(|token| GuestPath::absolute(token).ok())
                .map(|path| vec![path.as_str().to_owned()])
                .unwrap_or_default();
            return Ok(DesktopLaunchIntent {
                entrypoint: registered.entrypoint,
                arguments,
            });
        }
        let mut arguments = Vec::new();
        for token in tokens.iter().skip(1) {
            expand_desktop_argument(token, paths, &mut arguments)?;
        }
        Ok(DesktopLaunchIntent {
            entrypoint: registered.entrypoint,
            arguments,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchIntent {
    pub entrypoint: &'static str,
    pub path: GuestPath,
    pub media_type: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssociationError {
    UnsupportedMediaType,
    UnregisteredApplication,
}

impl fmt::Display for AssociationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedMediaType => formatter.write_str("no application handles this file"),
            Self::UnregisteredApplication => {
                formatter.write_str("the chosen application is not registered for this file")
            }
        }
    }
}

impl std::error::Error for AssociationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopEntryError {
    TooLarge,
    InvalidUtf8,
    NotApplication,
    Hidden,
    MissingUrl,
    InvalidUrl,
    MissingExec,
    InvalidExec,
    UnsupportedFieldCode,
    UnregisteredExecutable,
}

impl fmt::Display for DesktopEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "the desktop launcher exceeds its size limit",
            Self::InvalidUtf8 => "the desktop launcher is not valid UTF-8",
            Self::NotApplication => "the desktop entry is not an application launcher",
            Self::Hidden => "the desktop launcher is hidden",
            Self::MissingUrl => "the desktop link has no URL",
            Self::InvalidUrl => "the desktop link is not a local guest file URL",
            Self::MissingExec => "the desktop launcher has no Exec command",
            Self::InvalidExec => "the desktop launcher has an invalid Exec command",
            Self::UnsupportedFieldCode => "the desktop launcher uses an unsupported field code",
            Self::UnregisteredExecutable => {
                "the desktop launcher executable is not registered with RiceWM"
            }
        })
    }
}

impl std::error::Error for DesktopEntryError {}

fn local_file_url(url: &str) -> Result<GuestPath, DesktopEntryError> {
    let encoded = url
        .strip_prefix("file://")
        .filter(|path| path.starts_with('/') && !path.contains(['?', '#']))
        .ok_or(DesktopEntryError::InvalidUrl)?;
    let mut bytes = Vec::with_capacity(encoded.len());
    let encoded = encoded.as_bytes();
    let mut index = 0;
    while index < encoded.len() {
        if encoded[index] == b'%' {
            let high = encoded
                .get(index + 1)
                .and_then(|value| hex_digit(*value))
                .ok_or(DesktopEntryError::InvalidUrl)?;
            let low = encoded
                .get(index + 2)
                .and_then(|value| hex_digit(*value))
                .ok_or(DesktopEntryError::InvalidUrl)?;
            bytes.push((high << 4) | low);
            index += 3;
        } else {
            bytes.push(encoded[index]);
            index += 1;
        }
    }
    let path = core::str::from_utf8(&bytes).map_err(|_| DesktopEntryError::InvalidUrl)?;
    if path.contains(['\0', '\\']) {
        return Err(DesktopEntryError::InvalidUrl);
    }
    GuestPath::absolute(path).map_err(|_| DesktopEntryError::InvalidUrl)
}

const fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn tokenize_exec(exec: &str) -> Result<Vec<String>, DesktopEntryError> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for character in exec.chars() {
        if escaped {
            token.push(character);
            escaped = false;
        } else {
            match character {
                '\\' => escaped = true,
                '"' => quoted = !quoted,
                value if value.is_whitespace() && !quoted => {
                    if !token.is_empty() {
                        tokens.push(core::mem::take(&mut token));
                    }
                }
                value if value.is_control() => return Err(DesktopEntryError::InvalidExec),
                value => token.push(value),
            }
        }
    }
    if quoted || escaped {
        return Err(DesktopEntryError::InvalidExec);
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn expand_desktop_argument(
    token: &str,
    paths: &[GuestPath],
    output: &mut Vec<String>,
) -> Result<(), DesktopEntryError> {
    match token {
        "%f" | "%u" => {
            if let Some(path) = paths.first() {
                output.push(path.as_str().to_owned());
            }
        }
        "%F" | "%U" => output.extend(paths.iter().map(|path| path.as_str().to_owned())),
        "%i" | "%c" | "%k" => {}
        "%%" => output.push(String::from("%")),
        value if value.contains('%') => return Err(DesktopEntryError::UnsupportedFieldCode),
        value => output.push(value.to_owned()),
    }
    Ok(())
}

#[must_use]
pub fn infer_media_type(entry: &EntryMetadata) -> &str {
    let name_type = media_type_from_name(&entry.name);
    // Dedicated executable/package handlers validate actual bytes themselves.
    // A backend's generic MIME must not divert a recognized suffix into a
    // shell or an ordinary archive opener.
    if matches!(name_type, "application/x-msdownload" | "application/x-ricewm-app") {
        return name_type;
    }
    let supplied = entry
        .media_type
        .as_deref()
        .filter(|value| valid_media_type(value));
    match supplied {
        // Browser and remote mounts commonly report this generic value.  A
        // recognized extension is more useful and is still only a MIME hint;
        // the decoder independently probes the file bytes.
        Some("application/octet-stream") if name_type != "application/octet-stream" => name_type,
        Some("application/zip" | "application/x-ole-storage" | "application/x-cfb")
            if name_type.starts_with("application/vnd.") || name_type == "application/msword" => name_type,
        Some(value) => value,
        None => name_type,
    }
}

fn valid_media_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_MEDIA_TYPE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'+' | b'.'))
        && value.split_once('/').is_some()
}

fn media_matches(pattern: &str, media_type: &str) -> bool {
    pattern == media_type
        || pattern.strip_suffix("/*").is_some_and(|prefix| {
            media_type.starts_with(prefix) && media_type.as_bytes().get(prefix.len()) == Some(&b'/')
        })
}

fn media_type_from_name(name: &str) -> &'static str {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let folded = basename.to_ascii_lowercase();
    if matches!(folded.as_str(), "dockerfile" | "containerfile") {
        return "text/x-dockerfile";
    }
    if folded == "manifest.json" {
        return "application/manifest+json";
    }
    if folded == ".env" || folded.starts_with(".env.") {
        return "text/x-env";
    }
    let extension = folded.rsplit_once('.').map(|(_, extension)| extension);
    match extension {
        Some("theme") => "application/x-ricewm-theme",
        Some("riceapp") => "application/x-ricewm-app",
        Some("txt" | "log" | "nfo") => "text/plain",
        Some("md" | "markdown" | "mdown" | "mkd") => "text/markdown",
        Some("csv") => "text/csv",
        Some("tsv") => "text/tab-separated-values",
        Some("rs") => "text/rust",
        Some("c" | "h") => "text/x-c",
        Some("cc" | "cpp" | "cxx" | "hpp") => "text/x-c++",
        Some("js" | "mjs" | "cjs" | "jsx") => "application/javascript",
        Some("ts" | "tsx") => "application/typescript",
        Some("json") => "application/json",
        Some("toml") => "application/toml",
        Some("yaml" | "yml") => "application/yaml",
        Some("xml") => "application/xml",
        Some("webmanifest") => "application/manifest+json",
        Some("manifest") => "application/manifest+json",
        Some("env") => "text/x-env",
        Some("svg") => "image/svg+xml",
        Some("html" | "htm") => "text/html",
        Some("xhtml") => "application/xhtml+xml",
        Some("css") => "text/css",
        Some("scss") => "text/x-scss",
        Some("sass") => "text/x-sass",
        Some("less") => "text/x-less",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("avif") => "image/avif",
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("ogv" | "ogg") => "video/ogg",
        Some("mov" | "move") => "video/quicktime",
        Some("wmv") => "video/x-ms-wmv",
        Some("asf") => "video/x-ms-asf",
        Some("avi") => "video/x-msvideo",
        Some("mpeg" | "mpg") => "video/mpeg",
        Some("m2ts") => "video/mp2t",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("m4a") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("oga" | "opus") => "audio/ogg",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("tar") => "application/x-tar",
        Some("gz" | "tgz") => "application/gzip",
        Some("bz2") => "application/x-bzip2",
        Some("xz") => "application/x-xz",
        Some("zst") => "application/zstd",
        Some("7z") => "application/x-7z-compressed",
        Some("rar") => "application/x-rar-compressed",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("odt") => "application/vnd.oasis.opendocument.text",
        Some("rtf") => "application/rtf",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("potx") => "application/vnd.openxmlformats-officedocument.presentationml.template",
        Some("ppsx") => "application/vnd.openxmlformats-officedocument.presentationml.slideshow",
        Some("odp") => "application/vnd.oasis.opendocument.presentation",
        Some("otp") => "application/vnd.oasis.opendocument.presentation-template",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("ods") => "application/vnd.oasis.opendocument.spreadsheet",
        Some("pem" | "key") => "application/x-pem-file",
        Some("p12" | "pfx") => "application/pkcs12",
        Some("gpg" | "asc") => "application/pgp-keys",
        Some("wasm") => "application/wasm",
        Some("sh" | "bash" | "zsh" | "fish") => "text/x-shellscript",
        Some("exe") => "application/x-msdownload",
        Some("bat" | "cmd") => "application/x-rice-wine-batch",
        Some("appimage") => "application/x-executable",
        Some("deb") => "application/vnd.debian.binary-package",
        Some("rpm") => "application/x-rpm",
        Some("so" | "dll" | "dylib") => "application/x-sharedlib",
        Some("o" | "obj") => "application/x-object",
        Some("class") => "application/java-vm",
        Some("bin") => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> EntryMetadata {
        EntryMetadata::new(
            GuestPath::absolute(path).expect("path"),
            EntryKind::File,
            1,
            0,
        )
    }

    #[test]
    fn defaults_are_first_party_and_media_specific() {
        let registry = AssociationRegistry::first_party();
        assert_eq!(registry.default_for(&file("/code.rs")), Some(RICE_PAPER));
        assert_eq!(registry.default_for(&file("/photo.webp")), Some(RICE_VIEW));
        assert_eq!(
            registry.default_for(&file("/document.pdf")),
            Some(RICE_VIEW)
        );
        assert_eq!(
            registry.default_for(&file("/movie.webm")),
            Some(RICE_PLAYER)
        );
    }

    #[test]
    fn video_extensions_are_typed_and_associated_with_rice_player() {
        let registry = AssociationRegistry::first_party();
        for (name, media_type) in [
            ("/Videos/clip.avi", "video/x-msvideo"),
            ("/Videos/clip.mov", "video/quicktime"),
            ("/Videos/clip.move", "video/quicktime"),
            ("/Videos/clip.wmv", "video/x-ms-wmv"),
            ("/Videos/clip.asf", "video/x-ms-asf"),
        ] {
            let entry = file(name);
            assert_eq!(infer_media_type(&entry), media_type);
            assert_eq!(registry.default_for(&entry), Some(RICE_PLAYER));
        }
    }

    #[test]
    fn document_audio_and_archive_extensions_have_specific_mime_types() {
        for (name, expected) in [
            ("/Music/song.flac", "audio/flac"),
            ("/Data/table.csv", "text/csv"),
            ("/Docs/README.md", "text/markdown"),
            ("/Docs/report.pdf", "application/pdf"),
            (
                "/Docs/report.docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ),
            (
                "/Docs/slides.pptx",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ),
            (
                "/Docs/budget.xlsx",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
            ("/Docs/Template.POTX", "application/vnd.openxmlformats-officedocument.presentationml.template"),
            ("/Docs/Show.PPSX", "application/vnd.openxmlformats-officedocument.presentationml.slideshow"),
            ("/Docs/Template.OTP", "application/vnd.oasis.opendocument.presentation-template"),
            ("/Archives/source.tar", "application/x-tar"),
            ("/Config/compose.yaml", "application/yaml"),
            ("/Config/custom.theme", "application/x-ricewm-theme"),
            ("/Downloads/Construct.RICEAPP", "application/x-ricewm-app"),
            ("/Web/site.webmanifest", "application/manifest+json"),
            ("/Containers/Dockerfile", "text/x-dockerfile"),
            ("/Config/.env", "text/x-env"),
            ("/Config/.env.local", "text/x-env"),
            ("/Scripts/start.sh", "text/x-shellscript"),
            ("/Downloads/SETUP.EXE", "application/x-msdownload"),
            ("/Downloads/install.bat", "application/x-rice-wine-batch"),
            ("/Downloads/launch.CMD", "application/x-rice-wine-batch"),
        ] {
            assert_eq!(infer_media_type(&file(name)), expected, "{name}");
        }
    }

    #[test]
    fn recognized_extension_overrides_generic_mount_mime_type() {
        let entry = file("/Videos/clip.avi").with_media_type("application/octet-stream");
        assert_eq!(infer_media_type(&entry), "video/x-msvideo");
    }

    #[test]
    fn executable_suffix_overrides_posix_mount_mime_type() {
        let entry = file("/Downloads/PUTTY.EXE").with_media_type("application/x-executable");
        assert_eq!(infer_media_type(&entry), "application/x-msdownload");
    }

    #[test]
    fn package_suffix_overrides_generic_archive_metadata() {
        let entry = file("/Downloads/Construct.RICEAPP").with_media_type("application/zip");
        assert_eq!(infer_media_type(&entry), "application/x-ricewm-app");
    }

    #[test]
    fn nfo_documents_are_text_files_handled_by_rice_paper() {
        let entry = file("/Documents/release.nfo").with_media_type("application/octet-stream");
        assert_eq!(infer_media_type(&entry), "text/plain");
        assert_eq!(
            AssociationRegistry::first_party().default_for(&entry),
            Some(RICE_PAPER)
        );
    }

    #[test]
    fn chosen_launch_is_allowlisted_and_keeps_the_guest_path() {
        let registry = AssociationRegistry::first_party();
        let entry = file("/Pictures/photo.png");
        let launch = registry
            .launch(&entry, Some("rice-view"))
            .expect("registered viewer");
        assert_eq!(launch.entrypoint, "rice-view");
        assert_eq!(launch.path.as_str(), "/Pictures/photo.png");
        assert_eq!(launch.media_type, "image/png");
        assert_eq!(
            registry.launch(&entry, Some("bowl")),
            Err(AssociationError::UnregisteredApplication)
        );
    }

    #[test]
    fn invalid_adapter_media_type_falls_back_to_extension() {
        let entry = file("/notes.txt").with_media_type("not a mime type");
        assert_eq!(infer_media_type(&entry), "text/plain");
    }

    #[test]
    fn desktop_launchers_are_bounded_and_allowlisted_without_a_shell() {
        let path = GuestPath::absolute("/root/Documents/note.txt").expect("path");
        let registry = AssociationRegistry::first_party();
        let intent = registry
            .launch_desktop_entry(
                b"[Desktop Entry]\nType=Application\nExec=leafpad --new-window %f\n",
                &[path],
            )
            .expect("trusted launcher");
        assert_eq!(intent.entrypoint, "rice-paper");
        assert_eq!(
            intent.arguments,
            ["--new-window", "/root/Documents/note.txt"]
        );
        assert_eq!(
            registry.launch_desktop_entry(
                b"[Desktop Entry]\nType=Application\nExec=sh -c \"rm -rf /\"\n",
                &[],
            ),
            Err(DesktopEntryError::UnregisteredExecutable)
        );
        assert_eq!(
            registry.launch_desktop_entry(
                b"[Desktop Entry]\nType=Application\nExec=leafpad %Z\n",
                &[],
            ),
            Err(DesktopEntryError::UnsupportedFieldCode)
        );
        assert_eq!(
            registry
                .launch_desktop_entry(
                    b"[Desktop Entry]\nType=Application\nExec=pcmanfm --profile=ricewm --new-win /root/Documents\n",
                    &[],
                )
                .expect("legacy PCManFM desktop icon"),
            DesktopLaunchIntent {
                entrypoint: "rice-files",
                arguments: vec![String::from("/root/Documents")],
            }
        );
        assert_eq!(
            registry
                .launch_desktop_entry(
                    b"[Desktop Entry]\nType=Application\nExec=rice-player %f\n",
                    &[],
                )
                .expect("bundled native player launcher")
                .entrypoint,
            "rice-player"
        );
    }

    #[test]
    fn every_seeded_native_desktop_shortcut_is_allowlisted() {
        let registry = AssociationRegistry::first_party();
        for entrypoint in [
            "control-gallery",
            "rice-paper",
            "rice-files",
            "rice-view",
            "bowl",
            "rice-calc",
            "rice-settings",
            "rice-performance",
            "rice-setup",
            "rice-profiler",
            "rice-extensions",
            "rice-term",
            "rice-player",
        ] {
            let launcher =
                format!("[Desktop Entry]\nType=Application\nExec=/root/.local/bin/{entrypoint}\n");
            assert_eq!(
                registry
                    .launch_desktop_entry(launcher.as_bytes(), &[])
                    .expect("seeded native desktop shortcut")
                    .entrypoint,
                entrypoint,
            );
        }
    }

    #[test]
    fn desktop_links_open_only_normalized_local_guest_paths() {
        let registry = AssociationRegistry::first_party();
        assert_eq!(
            registry
                .launch_desktop_entry(
                    b"[Desktop Entry]\nType=Link\nName=My Project\nURL=file:///mnt/local/My%20Project\n",
                    &[],
                )
                .expect("local mount link"),
            DesktopLaunchIntent {
                entrypoint: "rice-files",
                arguments: vec![String::from("/mnt/local/My Project")],
            }
        );
        for entry in [
            b"[Desktop Entry]\nType=Link\nURL=https://example.com/\n".as_slice(),
            b"[Desktop Entry]\nType=Link\nURL=file://remote/mnt/local\n".as_slice(),
            b"[Desktop Entry]\nType=Link\nURL=file:///mnt/local/%ZZ\n".as_slice(),
            b"[Desktop Entry]\nType=Link\nURL=file:///mnt/local/a%00b\n".as_slice(),
        ] {
            assert_eq!(
                registry.launch_desktop_entry(entry, &[]),
                Err(DesktopEntryError::InvalidUrl)
            );
        }
    }
}
