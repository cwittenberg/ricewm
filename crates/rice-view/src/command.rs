use crate::{
    CacheScope, DecodeIntent, DecodeLimits, DirectoryImages, FitPolicy, ImageCandidate,
    Invalidation, InvalidationReason, Point, Rotation, Size, Source, ViewTransform, Zoom,
    ZoomError,
};
use core::fmt;

const MAX_LAUNCH_ITEMS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchPlan {
    pub sources: Vec<Source>,
    pub fullscreen: bool,
    pub slideshow: bool,
}

impl LaunchPlan {
    /// Parses the compatible viewer command-line subset.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] for invalid arguments or a missing source.
    pub fn parse<I, S>(arguments: I) -> Result<Self, LaunchError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut sources = Vec::new();
        let mut fullscreen = false;
        let mut slideshow = false;
        let mut options = true;
        for argument in arguments.into_iter().map(Into::into) {
            if options && argument == "--" {
                options = false;
            } else if options && matches!(argument.as_str(), "-f" | "--fullscreen") {
                fullscreen = true;
            } else if options && matches!(argument.as_str(), "-s" | "--slideshow") {
                slideshow = true;
            } else if options && matches!(argument.as_str(), "-h" | "--help" | "-v" | "--version") {
                return Err(LaunchError::InformationalOption);
            } else if options && argument.starts_with('-') {
                return Err(LaunchError::UnknownOption);
            } else {
                sources.push(Source::new(argument).map_err(|_| LaunchError::InvalidSource)?);
                if sources.len() > MAX_LAUNCH_ITEMS {
                    return Err(LaunchError::TooManySources);
                }
            }
        }
        if sources.is_empty() {
            return Err(LaunchError::MissingSource);
        }
        Ok(Self {
            sources,
            fullscreen,
            slideshow,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchError {
    MissingSource,
    InvalidSource,
    TooManySources,
    UnknownOption,
    InformationalOption,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    SetDirectory {
        entries: Vec<ImageCandidate>,
        selected: Source,
    },
    DirectoryChanged(Vec<ImageCandidate>),
    Next,
    Previous,
    ZoomIn(Point),
    ZoomOut(Point),
    ActualSize,
    Fit(FitPolicy),
    Pan(Point),
    Resize(Size),
    RotateLeft,
    RotateRight,
    RequestSaveRotation,
    ConfirmSaveRotation(bool),
    EnterFullscreen(Size),
    ExitFullscreen,
    RequestDelete,
    ConfirmDelete(bool),
    DeleteCompleted(DeleteOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteOutcome {
    Trashed(Source),
    Failed { source: Source, message: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Decode(DecodeIntent),
    SelectionChanged(Option<Source>),
    Render,
    EnterFullscreen,
    ExitFullscreen,
    ConfirmDelete(Source),
    Trash(Source),
    ConfirmDestructiveSave(Source),
    SaveRotation {
        source: Source,
        rotation: Rotation,
        /// The adapter must normalize EXIF exactly once before writing pixels.
        normalize_exif: bool,
    },
    Invalidate(Invalidation),
    ReportError(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewerState {
    pub directory: DirectoryImages,
    pub transform: Option<ViewTransform>,
    pub rotation: Rotation,
    pub fullscreen: bool,
    saved_window_transform: Option<ViewTransform>,
    pending_delete: Option<Source>,
    pending_save: Option<Source>,
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            directory: DirectoryImages::default(),
            transform: None,
            rotation: Rotation::None,
            fullscreen: false,
            saved_window_transform: None,
            pending_delete: None,
            pending_save: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reducer {
    pub state: ViewerState,
    decode_limits: DecodeLimits,
}

impl Reducer {
    #[must_use]
    pub fn new(decode_limits: DecodeLimits) -> Self {
        Self {
            state: ViewerState::default(),
            decode_limits,
        }
    }

    pub fn set_transform(&mut self, transform: ViewTransform) {
        self.state.transform = Some(transform);
    }

    /// Accepts decoded geometry for the current source and establishes the
    /// initial fit transform. Stale adapter completions cannot replace the
    /// current image.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::StaleAdapterResponse`] when `source` is no
    /// longer selected, or [`CommandError::InvalidGeometry`] for invalid
    /// transform geometry.
    pub fn accept_decoded(
        &mut self,
        source: &Source,
        image: Size,
        viewport: Size,
    ) -> Result<Vec<Effect>, CommandError> {
        if self.current_source().as_ref() != Some(source) {
            return Err(CommandError::StaleAdapterResponse);
        }
        self.state.transform = Some(
            ViewTransform::new(viewport, image, Zoom::Fit(FitPolicy::NeverUpscale))
                .map_err(CommandError::from)?,
        );
        Ok(vec![Effect::Render])
    }

    /// Reduces one command to state changes and host effects.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] for missing state, invalid geometry, or a stale
    /// adapter response.
    pub fn apply(&mut self, command: Command) -> Result<Vec<Effect>, CommandError> {
        match command {
            Command::SetDirectory { entries, selected } => {
                self.state.directory = DirectoryImages::new(entries);
                if !self.state.directory.select(&selected) {
                    return Err(CommandError::SelectionMissing);
                }
                self.reset_for_selection();
                Ok(self.selection_effects())
            }
            Command::DirectoryChanged(entries) => {
                let before = self.current_source();
                self.state.directory.reconcile(entries);
                let changed = before != self.current_source();
                if changed {
                    self.reset_for_selection();
                    Ok(self.selection_effects())
                } else {
                    Ok(vec![Effect::Render])
                }
            }
            Command::Next => {
                let before = self.current_source();
                self.state.directory.move_next();
                Ok(self.after_navigation(before.as_ref()))
            }
            Command::Previous => {
                let before = self.current_source();
                self.state.directory.move_previous();
                Ok(self.after_navigation(before.as_ref()))
            }
            Command::ZoomIn(focal) => self.transform(|view| view.zoom_in(focal)),
            Command::ZoomOut(focal) => self.transform(|view| view.zoom_out(focal)),
            Command::ActualSize => self.set_zoom(Zoom::Actual, self.viewport_center()?),
            Command::Fit(policy) => self.set_zoom(Zoom::Fit(policy), self.viewport_center()?),
            Command::Pan(delta) => {
                let view = self
                    .state
                    .transform
                    .as_mut()
                    .ok_or(CommandError::ImageNotReady)?;
                view.pan(delta)?;
                Ok(vec![Effect::Render])
            }
            Command::Resize(size) => self.transform(|view| view.resize(size)),
            Command::RotateLeft => {
                self.state.rotation = self.state.rotation.left();
                Ok(vec![Effect::Render])
            }
            Command::RotateRight => {
                self.state.rotation = self.state.rotation.right();
                Ok(vec![Effect::Render])
            }
            Command::RequestSaveRotation => self.request_save_rotation(),
            Command::ConfirmSaveRotation(confirmed) => self.confirm_save_rotation(confirmed),
            Command::EnterFullscreen(viewport) => self.enter_fullscreen(viewport),
            Command::ExitFullscreen => Ok(self.exit_fullscreen()),
            Command::RequestDelete => self.request_delete(),
            Command::ConfirmDelete(confirmed) => self.confirm_delete(confirmed),
            Command::DeleteCompleted(outcome) => self.delete_completed(outcome),
        }
    }

    fn transform(
        &mut self,
        operation: impl FnOnce(&mut ViewTransform) -> Result<(), ZoomError>,
    ) -> Result<Vec<Effect>, CommandError> {
        let view = self
            .state
            .transform
            .as_mut()
            .ok_or(CommandError::ImageNotReady)?;
        operation(view)?;
        Ok(vec![Effect::Render])
    }

    fn set_zoom(&mut self, zoom: Zoom, focal: Point) -> Result<Vec<Effect>, CommandError> {
        self.transform(|view| view.set_zoom(zoom, focal))
    }

    fn viewport_center(&self) -> Result<Point, CommandError> {
        Ok(self
            .state
            .transform
            .ok_or(CommandError::ImageNotReady)?
            .viewport_center())
    }

    fn request_save_rotation(&mut self) -> Result<Vec<Effect>, CommandError> {
        let source = self.current_source().ok_or(CommandError::NoSelection)?;
        if self.state.rotation == Rotation::None {
            return Ok(Vec::new());
        }
        self.state.pending_save = Some(source.clone());
        Ok(vec![Effect::ConfirmDestructiveSave(source)])
    }

    fn confirm_save_rotation(&mut self, confirmed: bool) -> Result<Vec<Effect>, CommandError> {
        let source = self
            .state
            .pending_save
            .take()
            .ok_or(CommandError::NoPendingSave)?;
        if !confirmed {
            return Ok(Vec::new());
        }
        Ok(vec![Effect::SaveRotation {
            source,
            rotation: self.state.rotation,
            normalize_exif: true,
        }])
    }

    fn enter_fullscreen(&mut self, viewport: Size) -> Result<Vec<Effect>, CommandError> {
        if self.state.fullscreen {
            return Ok(Vec::new());
        }
        self.state.saved_window_transform = self.state.transform;
        if let Some(transform) = &mut self.state.transform {
            transform.resize(viewport)?;
        }
        self.state.fullscreen = true;
        Ok(vec![Effect::EnterFullscreen, Effect::Render])
    }

    fn exit_fullscreen(&mut self) -> Vec<Effect> {
        if !self.state.fullscreen {
            return Vec::new();
        }
        self.state.fullscreen = false;
        if let Some(saved) = self.state.saved_window_transform.take() {
            self.state.transform = Some(saved);
        }
        vec![Effect::ExitFullscreen, Effect::Render]
    }

    fn request_delete(&mut self) -> Result<Vec<Effect>, CommandError> {
        let source = self.current_source().ok_or(CommandError::NoSelection)?;
        self.state.pending_delete = Some(source.clone());
        Ok(vec![Effect::ConfirmDelete(source)])
    }

    fn confirm_delete(&mut self, confirmed: bool) -> Result<Vec<Effect>, CommandError> {
        let source = self
            .state
            .pending_delete
            .clone()
            .ok_or(CommandError::NoPendingDelete)?;
        if confirmed {
            Ok(vec![Effect::Trash(source)])
        } else {
            self.state.pending_delete = None;
            Ok(Vec::new())
        }
    }

    fn delete_completed(&mut self, outcome: DeleteOutcome) -> Result<Vec<Effect>, CommandError> {
        let pending = self
            .state
            .pending_delete
            .clone()
            .ok_or(CommandError::NoPendingDelete)?;
        match outcome {
            DeleteOutcome::Trashed(source) if source == pending => {
                self.state.pending_delete = None;
                self.state.directory.remove_current();
                self.reset_for_selection();
                let mut effects = vec![Effect::Invalidate(Invalidation {
                    scope: CacheScope::Source(source),
                    reason: InvalidationReason::Trashed,
                })];
                effects.extend(self.selection_effects());
                Ok(effects)
            }
            DeleteOutcome::Failed { source, message } if source == pending => {
                self.state.pending_delete = None;
                Ok(vec![Effect::ReportError(bounded_message(message))])
            }
            _ => Err(CommandError::StaleAdapterResponse),
        }
    }

    fn after_navigation(&mut self, before: Option<&Source>) -> Vec<Effect> {
        if before == self.current_source().as_ref() {
            vec![Effect::Render]
        } else {
            self.reset_for_selection();
            self.selection_effects()
        }
    }

    fn reset_for_selection(&mut self) {
        self.state.transform = None;
        self.state.rotation = Rotation::None;
        self.state.pending_delete = None;
        self.state.pending_save = None;
    }

    fn selection_effects(&self) -> Vec<Effect> {
        let source = self.current_source();
        let mut effects = vec![Effect::SelectionChanged(source.clone())];
        if let Some(source) = source {
            effects.push(Effect::Decode(DecodeIntent {
                preferred_format: source
                    .extension()
                    .and_then(crate::ImageFormat::from_extension),
                source,
                limits: self.decode_limits,
            }));
        }
        effects
    }

    fn current_source(&self) -> Option<Source> {
        self.state
            .directory
            .current()
            .map(|entry| entry.source.clone())
    }
}

fn bounded_message(mut message: String) -> String {
    const LIMIT: usize = 1_024;
    if message.len() > LIMIT {
        let mut boundary = LIMIT;
        while !message.is_char_boundary(boundary) {
            boundary -= 1;
        }
        message.truncate(boundary);
    }
    message
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandError {
    SelectionMissing,
    NoSelection,
    ImageNotReady,
    NoPendingDelete,
    NoPendingSave,
    StaleAdapterResponse,
    InvalidGeometry,
}

impl From<ZoomError> for CommandError {
    fn from(_: ZoomError) -> Self {
        Self::InvalidGeometry
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CommandError {}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LaunchError {}
