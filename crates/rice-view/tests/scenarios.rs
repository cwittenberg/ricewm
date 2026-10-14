use rice_view::{
    CacheKey, CachePurpose, CacheScope, ClockError, Command, DecodeError, DecodeErrorKind,
    DecodeIntent, DecodeLimits, DecodedImage, DecoderAdapter, DeleteOutcome, DirectoryImages,
    Effect, ExifOrientation, FitPolicy, ImageCandidate, ImageFormat, ImageMetadata, Invalidation,
    InvalidationReason, LaunchError, LaunchPlan, PlaybackClock, PlaybackEvent, Point,
    ReconcileOutcome, Reducer, Rotation, Size, Source, SourceFingerprint, ThumbnailSize,
    ViewTransform, Visibility, Zoom,
};

fn source(value: &str) -> Source {
    Source::new(value).expect("fixture source")
}

fn candidate(value: &str) -> ImageCandidate {
    ImageCandidate::from_source(source(value)).expect("supported image fixture")
}

fn metadata(format: ImageFormat) -> ImageMetadata {
    ImageMetadata {
        format,
        width: 640,
        height: 480,
        has_alpha: true,
        frame_count: 1,
        orientation: ExifOrientation::Normal,
        decoded_bytes: 640 * 480 * 4,
    }
}

struct FakeDecoder;

impl DecoderAdapter for FakeDecoder {
    fn decode(&mut self, intent: &DecodeIntent) -> Result<DecodedImage, DecodeError> {
        Ok(DecodedImage {
            metadata: metadata(intent.preferred_format.unwrap_or(ImageFormat::Png)),
            handle: 7,
        })
    }
}

fn view() -> ViewTransform {
    ViewTransform::new(
        Size::new(800.0, 600.0).unwrap(),
        Size::new(1_600.0, 1_200.0).unwrap(),
        Zoom::Fit(FitPolicy::NeverUpscale),
    )
    .unwrap()
}

fn reducer_with_images() -> Reducer {
    let mut reducer = Reducer::new(DecodeLimits::default());
    reducer
        .apply(Command::SetDirectory {
            entries: vec![candidate("/pics/a.png"), candidate("/pics/b.jpg")],
            selected: source("/pics/a.png"),
        })
        .unwrap();
    reducer.set_transform(view());
    reducer
}

#[test]
fn view_001_validates_formats_metadata_and_adapter_results() {
    for (extension, expected) in [
        ("png", ImageFormat::Png),
        ("JPG", ImageFormat::Jpeg),
        ("webp", ImageFormat::WebP),
        ("gif", ImageFormat::Gif),
        ("bmp", ImageFormat::Bmp),
        ("svg", ImageFormat::Svg),
        ("pdf", ImageFormat::Pdf),
    ] {
        assert_eq!(ImageFormat::from_extension(extension), Some(expected));
        assert!(metadata(expected).validate(DecodeLimits::default()).is_ok());
    }

    let mut excessive = metadata(ImageFormat::Png);
    excessive.width = 40_000;
    assert!(excessive.validate(DecodeLimits::default()).is_err());

    let intent = DecodeIntent {
        source: source("/pics/example.png"),
        limits: DecodeLimits::default(),
        preferred_format: Some(ImageFormat::Png),
    };
    let decoded = FakeDecoder.decode(&intent).unwrap();
    assert_eq!(decoded.handle, 7);
    assert!(decoded.validate(intent.limits).is_ok());

    let error = DecodeError::new(DecodeErrorKind::Corrupt, "é".repeat(2_000));
    assert!(error.message().len() <= 1_024);
    assert!(error.message().is_char_boundary(error.message().len()));
}

#[test]
fn view_002_stable_order_and_reconciliation() {
    let mut images = DirectoryImages::new([
        candidate("/pics/z.png"),
        candidate("/pics/A.jpg"),
        candidate("/pics/b.webp"),
        candidate("/pics/A.jpg"),
    ]);
    assert_eq!(
        images
            .entries()
            .iter()
            .map(|entry| entry.source.as_str())
            .collect::<Vec<_>>(),
        ["/pics/A.jpg", "/pics/b.webp", "/pics/z.png"]
    );
    assert!(images.select(&source("/pics/b.webp")));
    assert_eq!(
        images.reconcile([
            candidate("/pics/a.jpg"),
            candidate("/pics/c.gif"),
            candidate("/pics/z.png"),
        ]),
        ReconcileOutcome::AdvancedAfterRemoval
    );
    assert_eq!(images.current().unwrap().source.as_str(), "/pics/c.gif");
    assert_eq!(images.move_next().unwrap().source.as_str(), "/pics/z.png");
    assert_eq!(
        images.move_previous().unwrap().source.as_str(),
        "/pics/c.gif"
    );
}

#[test]
fn view_003_zoom_fit_pan_resize_and_focal_point() {
    let mut transform = view();
    assert!((transform.scale() - 0.5).abs() < f64::EPSILON);
    let focal = Point::new(200.0, 150.0);
    let image_before = transform.viewport_to_image(focal);
    transform.zoom_in(focal).unwrap();
    let image_after = transform.viewport_to_image(focal);
    assert!((image_before.x - image_after.x).abs() < 0.000_1);
    assert!((image_before.y - image_after.y).abs() < 0.000_1);

    let center_before = transform.center();
    transform.pan(Point::new(25.0, -25.0)).unwrap();
    assert_ne!(transform.center(), center_before);
    transform
        .set_zoom(Zoom::Actual, transform.viewport_center())
        .unwrap();
    assert!((transform.scale() - 1.0).abs() < f64::EPSILON);

    let small = Size::new(100.0, 100.0).unwrap();
    let viewport = Size::new(500.0, 500.0).unwrap();
    let never_upscaled =
        ViewTransform::new(viewport, small, Zoom::Fit(FitPolicy::NeverUpscale)).unwrap();
    assert!((never_upscaled.scale() - 1.0).abs() < f64::EPSILON);
    transform.resize(Size::new(400.0, 300.0).unwrap()).unwrap();
}

#[test]
fn decoded_geometry_is_accepted_only_for_the_current_source() {
    let mut reducer = Reducer::new(DecodeLimits::default());
    reducer
        .apply(Command::SetDirectory {
            entries: vec![candidate("/pics/a.png"), candidate("/pics/b.jpg")],
            selected: source("/pics/a.png"),
        })
        .unwrap();
    assert!(reducer.state.transform.is_none());
    let effects = reducer
        .accept_decoded(
            &source("/pics/a.png"),
            Size::new(1_600.0, 900.0).unwrap(),
            Size::new(800.0, 600.0).unwrap(),
        )
        .unwrap();
    assert_eq!(effects, [Effect::Render]);
    assert!(reducer.state.transform.is_some());
    reducer.apply(Command::Next).unwrap();
    assert!(reducer.state.transform.is_none());
    assert_eq!(
        reducer.accept_decoded(
            &source("/pics/a.png"),
            Size::new(1_600.0, 900.0).unwrap(),
            Size::new(800.0, 600.0).unwrap(),
        ),
        Err(rice_view::CommandError::StaleAdapterResponse)
    );
}

#[test]
fn view_004_exif_and_user_rotation_are_not_double_applied() {
    let mut image = metadata(ImageFormat::Jpeg);
    image.width = 1_200;
    image.height = 800;
    image.orientation = ExifOrientation::Rotate90;
    assert_eq!(image.oriented_dimensions(Rotation::None), (800, 1_200));
    assert_eq!(image.oriented_dimensions(Rotation::Right90), (1_200, 800));
    assert_eq!(
        ExifOrientation::MirrorVertical.pixel_transform(Rotation::Right90),
        rice_view::PixelTransform {
            mirror_horizontal: true,
            quarter_turns_clockwise: 3,
        }
    );

    let mut reducer = reducer_with_images();
    reducer.apply(Command::RotateRight).unwrap();
    assert_eq!(reducer.state.rotation, Rotation::Right90);
    assert!(matches!(
        reducer.apply(Command::RequestSaveRotation).unwrap()[0],
        Effect::ConfirmDestructiveSave(_)
    ));
    assert!(matches!(
        reducer.apply(Command::ConfirmSaveRotation(true)).unwrap()[0],
        Effect::SaveRotation {
            rotation: Rotation::Right90,
            normalize_exif: true,
            ..
        }
    ));
}

#[test]
fn view_005_fullscreen_restores_exact_window_transform() {
    let mut reducer = reducer_with_images();
    let original = reducer.state.transform;
    let effects = reducer
        .apply(Command::EnterFullscreen(
            Size::new(1_920.0, 1_080.0).unwrap(),
        ))
        .unwrap();
    assert!(reducer.state.fullscreen);
    assert_eq!(effects[0], Effect::EnterFullscreen);
    assert_ne!(reducer.state.transform, original);
    reducer.apply(Command::ExitFullscreen).unwrap();
    assert!(!reducer.state.fullscreen);
    assert_eq!(reducer.state.transform, original);
}

#[test]
fn view_006_animation_pause_resume_and_hidden_throttle() {
    let mut clock = PlaybackClock::new(100).unwrap();
    assert_eq!(clock.tick(0).unwrap(), PlaybackEvent::NoChange);
    assert_eq!(
        clock.tick(250).unwrap(),
        PlaybackEvent::Advanced { steps: 2 }
    );
    clock.pause();
    assert_eq!(clock.tick(1_000).unwrap(), PlaybackEvent::NoChange);
    clock.resume(1_000);
    clock.set_visibility(Visibility::Hidden, 1_000);
    assert_eq!(clock.tick(60_999).unwrap(), PlaybackEvent::NoChange);
    assert_eq!(
        clock.tick(61_000).unwrap(),
        PlaybackEvent::Advanced { steps: 1 }
    );
    assert_eq!(clock.tick(60_000), Err(ClockError::NonMonotonic));
}

#[test]
fn view_007_delete_advances_only_after_success() {
    let mut reducer = reducer_with_images();
    assert!(matches!(
        reducer.apply(Command::RequestDelete).unwrap()[0],
        Effect::ConfirmDelete(_)
    ));
    assert!(matches!(
        reducer.apply(Command::ConfirmDelete(true)).unwrap()[0],
        Effect::Trash(_)
    ));
    let failure = reducer
        .apply(Command::DeleteCompleted(DeleteOutcome::Failed {
            source: source("/pics/a.png"),
            message: "permission denied".into(),
        }))
        .unwrap();
    assert_eq!(
        reducer.state.directory.current().unwrap().source.as_str(),
        "/pics/a.png"
    );
    assert_eq!(failure, [Effect::ReportError("permission denied".into())]);

    reducer.apply(Command::RequestDelete).unwrap();
    reducer.apply(Command::ConfirmDelete(true)).unwrap();
    let success = reducer
        .apply(Command::DeleteCompleted(DeleteOutcome::Trashed(source(
            "/pics/a.png",
        ))))
        .unwrap();
    assert_eq!(
        reducer.state.directory.current().unwrap().source.as_str(),
        "/pics/b.jpg"
    );
    assert!(matches!(success[0], Effect::Invalidate(_)));
    assert!(
        success
            .iter()
            .any(|effect| matches!(effect, Effect::Decode(_)))
    );
}

#[test]
fn view_008_cache_key_is_shared_and_precisely_invalidated() {
    let key = CacheKey {
        source: source("/pics/a.png"),
        fingerprint: SourceFingerprint {
            identity: 42,
            modified_ms: 100,
            byte_len: 200,
        },
        purpose: CachePurpose::Thumbnail(ThumbnailSize::new(128, 128).unwrap()),
        exif_orientation: ExifOrientation::Normal,
        user_rotation: Rotation::None,
        codec_version: 3,
    };
    let source_invalidation = Invalidation {
        scope: CacheScope::Source(source("/pics/a.png")),
        reason: InvalidationReason::ContentChanged,
    };
    assert!(source_invalidation.matches(&key));
    assert!(
        !Invalidation {
            scope: CacheScope::CodecVersion(2),
            reason: InvalidationReason::CodecUpgraded,
        }
        .matches(&key)
    );
}

#[test]
fn view_009_launch_plan_accepts_paths_file_uris_and_flags() {
    let plan = LaunchPlan::parse([
        "--fullscreen",
        "--slideshow",
        "/pics/a.png",
        "file:///pics/b.jpg",
    ])
    .unwrap();
    assert!(plan.fullscreen);
    assert!(plan.slideshow);
    assert_eq!(plan.sources.len(), 2);
    assert_eq!(plan.sources[1].as_str(), "/pics/b.jpg");
    assert_eq!(
        LaunchPlan::parse(["file://localhost/pics/My%20Photo.png"])
            .unwrap()
            .sources[0]
            .as_str(),
        "/pics/My Photo.png"
    );
    assert_eq!(
        LaunchPlan::parse(["file://remote/pics/a.png"]),
        Err(LaunchError::InvalidSource)
    );
    assert_eq!(
        LaunchPlan::parse(["file:///pics/%2e%2e/a.png"]),
        Err(LaunchError::InvalidSource)
    );
    assert_eq!(
        LaunchPlan::parse(["https://host/image.png"]),
        Err(LaunchError::InvalidSource)
    );
    assert_eq!(
        LaunchPlan::parse(["--wat", "/pics/a.png"]),
        Err(LaunchError::UnknownOption)
    );
    assert_eq!(
        LaunchPlan::parse(std::iter::empty::<&str>()),
        Err(LaunchError::MissingSource)
    );
}

#[test]
fn stale_delete_completion_cannot_remove_another_image() {
    let mut reducer = reducer_with_images();
    reducer.apply(Command::RequestDelete).unwrap();
    let result = reducer.apply(Command::DeleteCompleted(DeleteOutcome::Trashed(source(
        "/pics/b.jpg",
    ))));
    assert!(result.is_err());
    assert_eq!(reducer.state.directory.entries().len(), 2);
}
