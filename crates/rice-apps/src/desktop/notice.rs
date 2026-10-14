//! Retained ownership for visible notices. Filesystem diagnostics and filenames stay literal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DesktopNotice {
    Literal(String),
    Key(&'static str),
    Template {
        key: &'static str,
        args: Vec<(&'static str, DesktopNotice)>,
    },
}
impl DesktopNotice {
    pub(super) fn key(key: &'static str) -> Self {
        Self::Key(key)
    }
    pub(super) fn literal(value: impl Into<String>) -> Self {
        Self::Literal(value.into())
    }
    pub(super) fn template(key: &'static str, args: Vec<(&'static str, Self)>) -> Self {
        Self::Template { key, args }
    }
    pub(super) fn error_detail(value: &str) -> Self {
        if value.trim().is_empty() {
            Self::key("files.recovery.generic")
        } else {
            Self::literal(value)
        }
    }
    pub(super) fn render(&self) -> String {
        match self {
            Self::Literal(value) => value.clone(),
            Self::Key(key) => rice_i18n::t(key).to_owned(),
            Self::Template { key, args } => {
                let rendered = args
                    .iter()
                    .map(|(name, value)| (*name, value.render()))
                    .collect::<Vec<_>>();
                let args = rendered
                    .iter()
                    .map(|(name, value)| (*name, value.as_str()))
                    .collect::<Vec<_>>();
                rice_i18n::format(key, &args)
            }
        }
    }
}
