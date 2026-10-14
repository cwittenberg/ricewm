//! Data-only presentation metadata for native text editors.

const PREFIX: &str = "editor:";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorWrap {
    None,
    Word,
    Character,
}

impl EditorWrap {
    const fn token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Word => "word",
            Self::Character => "char",
        }
    }
}

/// Fixed plain-text viewport metadata; offsets are pixels relative to the
/// bounded text window, never a translation of the text control itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditorViewport {
    pub offset: u32,
    pub extent: u32,
    pub digits: u32,
    pub continuation: bool,
    pub caret_visible: bool,
    pub caret_upstream: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditorText<'a> {
    pub viewport: Option<EditorViewport>,
    pub language: &'a str,
    pub syntax_style: &'a str,
    pub wrap: EditorWrap,
    pub line_numbers: bool,
    pub highlight_current_line: bool,
    pub caret: Option<usize>,
    pub anchor: Option<usize>,
    pub line_number_start: usize,
    /// Explicit editor font size in logical pixels.
    pub font_size: Option<u16>,
    pub force_dark_mode: bool,
    pub document: &'a str,
}

impl<'a> EditorText<'a> {
    #[must_use]
    pub fn parse(text: &'a str) -> Option<Self> {
        let (header, document) = text.split_once('\n')?;
        let metadata = header.strip_prefix(PREFIX)?;
        let mut fields = metadata.split(':');
        let encoded_language = fields.next()?;
        let (encoded_language, force_dark_mode) = encoded_language
            .strip_suffix("~dark")
            .map_or((encoded_language, false), |language| (language, true));
        let sized_language = encoded_language
            .rsplit_once("~z")
            .filter(|(language, _)| language == &"rich" || language.starts_with("rich~"));
        let (language, encoded_font_size) = match sized_language {
            Some((language, value)) => (
                language,
                Some(
                    value
                        .parse::<u16>()
                        .ok()
                        .filter(|size| (6..=96).contains(size))?,
                ),
            ),
            None => match encoded_language.rsplit_once('@') {
                Some((language, value)) if !language.is_empty() => (
                    language,
                    Some(
                        value
                            .parse::<u16>()
                            .ok()
                            .filter(|size| (6..=96).contains(size))?,
                    ),
                ),
                _ => (encoded_language, None),
            },
        };
        let (language, viewport) = if let Some((language, value)) = language.rsplit_once("~V") {
            let values = value
                .split(',')
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            if !(5..=6).contains(&values.len())
                || values[2] == 0
                || values[2] > 20
                || values[3] > 1
                || values[4] > 1
                || values.get(5).is_some_and(|v| *v > 1)
            {
                return None;
            }
            (
                language,
                Some(EditorViewport {
                    offset: values[0],
                    extent: values[1],
                    digits: values[2],
                    continuation: values[3] != 0,
                    caret_visible: values[4] != 0,
                    caret_upstream: values.get(5) == Some(&1),
                }),
            )
        } else {
            (language, None)
        };
        let wrap = fields.next()?;
        let line_numbers = match fields.next()? {
            "lines" => true,
            "nolines" => false,
            _ => return None,
        };
        let caret = fields.next().map(str::parse).transpose().ok()?;
        let anchor = fields.next().map(str::parse).transpose().ok()?;
        let highlight_current_line = match fields.next() {
            Some("currentline") => true,
            Some("nocurrentline") => false,
            Some(_) => return None,
            None => false,
        };
        let line_number_start = fields
            .next()
            .map(str::parse)
            .transpose()
            .ok()?
            .unwrap_or(1)
            .max(1);
        let syntax_style = fields.next().unwrap_or("adaptive");
        if !matches!(syntax_style, "adaptive" | "vibrant" | "soft" | "mono") {
            return None;
        }
        // Accept the briefly shipped trailing field so an in-flight older
        // RicePaper build can still render against a newer display module.
        // New producers encode the size in the otherwise open language token;
        // older display modules therefore keep parsing the presentation too.
        let trailing_font_size = match fields.next() {
            Some(value) => Some(
                value
                    .parse::<u16>()
                    .ok()
                    .filter(|size| (6..=96).contains(size))?,
            ),
            None => None,
        };
        let font_size = encoded_font_size.or(trailing_font_size);
        if fields.next().is_some() || anchor.is_some() != caret.is_some() {
            return None;
        }
        let wrap = match wrap {
            "none" => EditorWrap::None,
            "word" => EditorWrap::Word,
            "char" => EditorWrap::Character,
            _ => return None,
        };
        Some(Self {
            viewport,
            language,
            syntax_style,
            wrap,
            line_numbers,
            highlight_current_line,
            caret,
            anchor,
            line_number_start,
            font_size,
            force_dark_mode,
            document,
        })
    }
}

#[must_use]
pub fn editor_text(language: &str, wrap: EditorWrap, line_numbers: bool, document: &str) -> String {
    let lines = if line_numbers { "lines" } else { "nolines" };
    format!("{PREFIX}{language}:{}:{lines}\n{document}", wrap.token())
}

/// Encodes editor presentation plus character-based selection offsets.
#[must_use]
pub fn editor_text_selection(
    language: &str,
    wrap: EditorWrap,
    line_numbers: bool,
    caret: usize,
    anchor: usize,
    document: &str,
) -> String {
    let lines = if line_numbers { "lines" } else { "nolines" };
    format!(
        "{PREFIX}{language}:{}:{lines}:{caret}:{anchor}\n{document}",
        wrap.token()
    )
}

/// Encodes editor presentation, selection offsets, and current-line visibility.
#[must_use]
pub fn editor_text_selection_highlighted(
    language: &str,
    wrap: EditorWrap,
    line_numbers: bool,
    caret: usize,
    anchor: usize,
    highlight_current_line: bool,
    document: &str,
) -> String {
    let lines = if line_numbers { "lines" } else { "nolines" };
    let current_line = if highlight_current_line {
        "currentline"
    } else {
        "nocurrentline"
    };
    format!(
        "{PREFIX}{language}:{}:{lines}:{caret}:{anchor}:{current_line}\n{document}",
        wrap.token()
    )
}

/// Encodes a bounded editor window while retaining the original document's
/// one-based line-number origin.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn editor_text_windowed_highlighted(
    language: &str,
    wrap: EditorWrap,
    line_numbers: bool,
    caret: usize,
    anchor: usize,
    highlight_current_line: bool,
    line_number_start: usize,
    document: &str,
) -> String {
    let lines = if line_numbers { "lines" } else { "nolines" };
    let current_line = if highlight_current_line {
        "currentline"
    } else {
        "nocurrentline"
    };
    format!(
        "{PREFIX}{language}:{}:{lines}:{caret}:{anchor}:{current_line}:{}\n{document}",
        wrap.token(),
        line_number_start.max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_metadata_roundtrips_without_changing_document_bytes() {
        let encoded = editor_text("rust", EditorWrap::Word, true, "fn main() {}\n");
        assert_eq!(
            EditorText::parse(&encoded),
            Some(EditorText {
                viewport: None,
                language: "rust",
                syntax_style: "adaptive",
                wrap: EditorWrap::Word,
                line_numbers: true,
                highlight_current_line: false,
                caret: None,
                anchor: None,
                line_number_start: 1,
                font_size: None,
                force_dark_mode: false,
                document: "fn main() {}\n",
            })
        );
        assert_eq!(EditorText::parse("plain text"), None);
    }

    #[test]
    fn editor_selection_metadata_roundtrips() {
        let encoded = editor_text_selection("plain", EditorWrap::None, false, 5, 2, "hello");
        let parsed = EditorText::parse(&encoded).expect("selection presentation");
        assert_eq!((parsed.caret, parsed.anchor), (Some(5), Some(2)));
        assert_eq!(parsed.document, "hello");
    }

    #[test]
    fn editor_font_size_is_bounded_metadata_not_document_text() {
        let parsed =
            EditorText::parse("editor:plain@18:none:nolines:2:2:currentline:1:adaptive\nhello")
                .expect("sized editor metadata");
        assert_eq!(parsed.font_size, Some(18));
        assert_eq!(parsed.document, "hello");
        assert!(
            EditorText::parse("editor:plain@200:none:nolines:2:2:currentline:1:adaptive\nhello",)
                .is_none()
        );
        let transitional =
            EditorText::parse("editor:plain:none:nolines:2:2:currentline:1:adaptive:18\nhello")
                .expect("transitional sized editor metadata");
        assert_eq!(transitional.font_size, Some(18));
        let rich = EditorText::parse(
            "editor:rich~u0-5~s6-11~z18:word:nolines:2:2:currentline:1:adaptive\nhello world",
        )
        .expect("rich editor zoom metadata");
        assert_eq!(rich.language, "rich~u0-5~s6-11");
        assert_eq!(rich.font_size, Some(18));

        let forced_dark = EditorText::parse(
            "editor:rich~u0-5~z18~dark:word:lines:2:2:currentline:1:adaptive\nhello",
        )
        .expect("forced dark editor metadata");
        assert_eq!(forced_dark.language, "rich~u0-5");
        assert_eq!(forced_dark.font_size, Some(18));
        assert!(forced_dark.force_dark_mode);
        assert_eq!(forced_dark.document, "hello");
    }

    #[test]
    fn current_line_highlight_metadata_roundtrips() {
        let encoded = editor_text_selection_highlighted(
            "javascript",
            EditorWrap::None,
            true,
            5,
            2,
            true,
            "hello",
        );
        let parsed = EditorText::parse(&encoded).expect("highlighted editor presentation");
        assert!(parsed.highlight_current_line);
        assert_eq!((parsed.caret, parsed.anchor), (Some(5), Some(2)));
        assert_eq!(parsed.line_number_start, 1);
    }

    #[test]
    fn fixed_viewport_metadata_is_separate_from_language_font_and_document() {
        let encoded =
            "editor:json~V35,9000,4,1,0,1@18~dark:word:lines:9:9:currentline:42:adaptive\n{}";
        let parsed = EditorText::parse(encoded).unwrap();
        assert_eq!(parsed.language, "json");
        assert_eq!(parsed.font_size, Some(18));
        assert!(parsed.force_dark_mode);
        assert_eq!(parsed.document, "{}");
        assert_eq!(
            parsed.viewport,
            Some(EditorViewport {
                offset: 35,
                extent: 9000,
                digits: 4,
                continuation: true,
                caret_visible: false,
                caret_upstream: true
            })
        );
        for token in [
            "0,100,0,0,1",
            "0,100,4,2,1",
            "0,100,4,0,1,2",
            "0,100,4",
            "x,100,4,0,1",
        ] {
            assert!(
                EditorText::parse(&format!(
                    "editor:json~V{token}@12:word:lines:0:0:currentline:1:adaptive\n{{}}"
                ))
                .is_none()
            );
        }
    }

    #[test]
    fn windowed_metadata_retains_the_original_line_number() {
        let encoded = editor_text_windowed_highlighted(
            "html",
            EditorWrap::None,
            true,
            2,
            2,
            true,
            401,
            "<p>window</p>",
        );
        let parsed = EditorText::parse(&encoded).expect("windowed editor presentation");
        assert_eq!(parsed.line_number_start, 401);
        assert_eq!(parsed.document, "<p>window</p>");
    }
}
