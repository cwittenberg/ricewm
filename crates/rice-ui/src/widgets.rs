use crate::WidgetId;

/// Stable semantic icon keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum IconId {
    Application = 1,
    Document = 2,
    Folder = 3,
    FolderOpen = 4,
    Home = 5,
    Trash = 6,
    DocumentNew = 10,
    DocumentOpen = 11,
    DocumentSave = 12,
    EditUndo = 20,
    EditRedo = 21,
    EditCut = 22,
    EditCopy = 23,
    EditPaste = 24,
    EditDelete = 25,
    Search = 30,
    GoBack = 31,
    GoForward = 32,
    GoUp = 33,
    Refresh = 34,
    ViewList = 40,
    ViewGrid = 41,
    Settings = 50,
    Information = 51,
    Warning = 52,
    Error = 53,
    Close = 54,
    Minimize = 55,
    Maximize = 56,
    Restore = 57,
    TrashFull = 58,
    Calculator = 60,
    TextEditor = 61,
    Files = 62,
    ImageViewer = 63,
    Drive = 64,
    Network = 65,
    Lock = 66,
    DocumentSaveAs = 67,
    EditFind = 68,
    EditRename = 69,
    Desktop = 70,
    Image = 71,
    Terminal = 72,
    ChevronUp = 73,
    ChevronDown = 74,
    Menu = 75,
    ObjectAdd = 76,
    ObjectRemove = 77,
    SelectionCheck = 78,
    ViewDetail = 79,
    Link = 80,
    MediaSeekBack = 81,
    MediaPlay = 82,
    MediaPause = 83,
    MediaSeekForward = 84,
    AudioVolume = 85,
    AudioMuted = 86,
    Fullscreen = 87,
    Calendar = 88,
    Audio = 89,
    Binary = 90,
    Css = 91,
    Csv = 92,
    WordDocument = 93,
    Docker = 94,
    Documents = 95,
    Downloads = 96,
    Env = 97,
    Archive = 98,
    Html = 99,
    JavaScript = 100,
    Manifest = 101,
    Markdown = 102,
    Pdf = 103,
    Presentation = 104,
    Program = 105,
    Secret = 106,
    Text = 107,
    Video = 108,
    Spreadsheet = 109,
    Yaml = 110,
    Pictures = 111,
    NetworkMounts = 112,
    /// RiceWM product identity. Use only where the shell or an application is
    /// identifying RiceWM itself; ordinary application actions remain semantic.
    RicewmLogo = 113,
    WelcomeEveryday = 114,
    WelcomeCreate = 115,
    WelcomeConnect = 116,
    WelcomeExtensions = 117,
    WelcomePackages = 118,
    WelcomePrivacy = 119,
    /// Rich first-run illustration. Kept as a semantic fallback until it is
    /// promoted into the canonical icon font.
    WelcomeHeroColor = 120,
    /// Bowl web browser application identity from `ricewm.woff2`.
    Bowl = 121,
    /// Welcome-card speed mark. SVG fallback until promoted into ricewm.woff2.
    WelcomeSpeed = 122,
    /// Welcome-card full desktop mark. SVG fallback until promoted into ricewm.woff2.
    WelcomeDesktop = 123,
    /// Welcome-card privacy mark. SVG fallback until promoted into ricewm.woff2.
    WelcomePrivate = 124,
    /// Colored RiceWM identity used by the first-run hero.
    WelcomeBrand = 125,
    FormatBold = 126,
    FormatItalic = 127,
    FormatUnderline = 128,
    FormatStrike = 129,
    MailUnread = 130,
    MailRead = 131,
    /// Decorative rice-grain mark used independently of application identity.
    Grain = 132,
    Star = 133,
    /// Today and day-planning destination.
    Sun = 600,
    /// An importance action whose task is not yet starred.
    StarOutline = 601,
    /// Work lists and workspaces.
    Briefcase = 602,
    /// Cascading-menu indicator. SVG fallback until promoted into ricewm.woff2.
    SubmenuRight = 134,
    /// Viewport-flipped cascading-menu indicator.
    SubmenuLeft = 135,
    /// Microsoft Windows application identity. This is a platform badge, not
    /// a generic executable or application action.
    WindowsLogo = 136,
    /// Printer device/folder identity used by classic shell views.
    Printer = 137,
    /// Mail-client application identity used by Box.
    Inbox = 138,
    /// Contact/person application identity used by Card.
    User = 139,
    /// Chat application identity used by Paddy.
    Comment = 140,
    /// Linux application identity. This is used for APT source presentation,
    /// not as a generic application action.
    LinuxLogo = 141,
    /// Snap application identity. This is a package-source badge, not a
    /// generic application action.
    SnapLogo = 142,
    /// Conventional power/session action.
    Power = 143,
    /// Leave the current desktop session.
    Logout = 144,
    /// Restart the current machine or environment.
    Reboot = 145,
    /// Screen capture action used by the RiceWM shell.
    Camera = 147,
    Attachment = 148,
    MailReply = 149,
    MailForward = 150,
    FormatAlignLeft = 151,
    FormatAlignCenter = 152,
    FormatAlignRight = 153,
    FormatAlignJustify = 154,
    FormatBullets = 155,
    FormatNumbering = 156,
    FormatFontSmaller = 157,
    FormatFontLarger = 158,
    GoogleDrive = 159,
    Nextcloud = 160,
    S3Storage = 161,
    DataFilter = 162,
    DataFilterClear = 163,
    SortAscending = 164,
    SortDescending = 165,
    FormulaFunction = 166,
    FormulaSum = 167,
    FormulaLookup = 168,
    FillDown = 169,
    FillRight = 170,
    PasteValues = 171,
    PasteFormulas = 172,
    RowHeight = 173,
    ColumnWidth = 174,
    FormatNumber = 175,
    FormatPercent = 176,
    FormatCurrency = 177,
    InsertRows = 178,
    InsertColumns = 179,
    DeleteRows = 180,
    DeleteColumns = 181,
    Shapes = 182,
    Line = 183,
    Rectangle = 184,
    Ellipse = 185,
    FillColor = 186,
    OutlineColor = 187,
    TextColor = 188,
    Arrange = 189,
    AlignObjects = 190,
    DistributeHorizontal = 191,
    DistributeVertical = 192,
    Crop = 193,
    TextBox = 194,
    Rotate = 195,
    BringForward = 196,
    SendBackward = 197,
    ShapeRectangle = 198,
    ShapeRoundedRectangle = 199,
    ShapeEllipse = 200,
    ShapeTriangle = 201,
    ShapeRightTriangle = 202,
    ShapeDiamond = 203,
    ShapeParallelogram = 204,
    ShapeTrapezoid = 205,
    ShapeCross = 206,
    ShapeHeart = 207,
    ShapePentagon = 208,
    ShapeHexagon = 209,
    ShapeHeptagon = 210,
    ShapeOctagon = 211,
    ShapeDecagon = 212,
    ShapeDodecagon = 213,
    ShapeRightArrow = 214,
    ShapeDownArrow = 215,
    ShapeLeftArrow = 216,
    ShapeUpArrow = 217,
    ShapeLeftRightArrow = 218,
    ShapeUpDownArrow = 219,
    ShapeChevron = 220,
    ShapeHomePlate = 221,
    ShapeStar4 = 222,
    ShapeStar5 = 223,
    ShapeStar6 = 224,
    ShapeStar8 = 225,
    ShapeStar10 = 226,
    ShapeStar12 = 227,
    ShapeSpeechCallout = 228,
    ShapeDocument = 229,
    ObjectAlignLeft = 230,
    ObjectAlignCenter = 231,
    ObjectAlignRight = 232,
    ObjectAlignTop = 233,
    ObjectAlignMiddle = 234,
    ObjectAlignBottom = 235,
    CellStyles = 236,
    CellBorders = 237,
    FormatCells = 238,
    ObjectGroup = 239,
    ObjectUngroup = 240,
    ChartColumn = 241,
    ChartLine = 242,
    ChartFormat = 243,
    ChartData = 244,
    ChartBar = 245,
    ChartArea = 246,
    ChartPie = 247,
    ChartDoughnut = 248,
    ChartScatter = 249,
    ChartBubble = 250,
    ChartRadar = 251,
    ChartStock = 252,
    ChartCombo = 253,
    ChartSurface = 254,
    ChartStatistical = 255,
    ChartWaterfall = 256,
    ChartFunnel = 512,
    ChartHierarchy = 513,
    FormatPainter = 514,
    FormatLineSpacing = 515,
    ZoomIn = 516,
    ZoomOut = 517,
    DrawPolygon = 518,
    DrawPolyline = 519,
    ShapeSnip1Rect = 520,
    ShapeSnip2SameRect = 521,
    ShapeSnip2DiagRect = 522,
    ShapeRound1Rect = 523,
    ShapeRound2SameRect = 524,
    ShapeRound2DiagRect = 525,
    ShapeSnipRoundRect = 526,
    ShapeBevel = 527,
    ShapeCylinder = 528,
    ShapeCube = 529,
    ShapeCorner = 530,
    ShapeFoldedCorner = 531,
    ShapeLightningBolt = 532,
    ShapeMoon = 533,
    ShapeSun = 534,
    ShapeTeardrop = 535,
    ShapeDonut = 536,
    ShapeQuadArrow = 537,
    ShapeLeftRightUpArrow = 538,
    ShapeBentArrow = 539,
    ShapeUturnArrow = 540,
    ShapeLeftUpArrow = 541,
    ShapeNotchedRightArrow = 542,
    ShapeMathPlus = 543,
    ShapeMathMinus = 544,
    ShapeMathMultiply = 545,
    ShapeMathDivide = 546,
    ShapeMathEqual = 547,
    ShapeMathNotEqual = 548,
    ShapeFlowChartPredefinedProcess = 549,
    ShapeFlowChartInternalStorage = 550,
    ShapeFlowChartTerminator = 551,
    ShapeFlowChartPreparation = 552,
    ShapeFlowChartManualInput = 553,
    ShapeFlowChartManualOperation = 554,
    ShapeFlowChartOffpageConnector = 555,
    ShapeFlowChartDelay = 556,
    ShapeFlowChartDisplay = 557,
    ShapeStar16 = 558,
    ShapeStar24 = 559,
    ShapeStar32 = 560,
    ShapeRoundCallout = 561,
    ShapeEllipseCallout = 562,
    SpreadsheetFile = 563,
    PresentationFile = 564,
    FormatWrapText = 565,
    FormatDecimalIncrease = 566,
    FormatDecimalDecrease = 567,
    DataRemoveDuplicates = 568,
    DataTextToColumns = 569,
    ShapeShadow = 570,
    Accessibility = 571,
    Appearance = 572,
    Taskbar = 573,
    Extensions = 574,
    Computer = 575,
    Brain = 576,
    Clock = 577,

    // Shortcut variants preserve the target's semantic icon while asking the
    // renderer to add the conventional shortcut badge.  They intentionally
    // occupy a separate wire range so ordinary theme icon mappings continue
    // to receive the unwrapped base icon.
    ShortcutSpreadsheetFile = 0x333,
    ShortcutPresentationFile = 0x334,
    ShortcutApplication = 0x101,
    ShortcutDocument = 0x102,
    ShortcutFolder = 0x103,
    ShortcutHome = 0x105,
    ShortcutDesktop = 0x146,
    ShortcutImage = 0x147,
    ShortcutAudio = 0x159,
    ShortcutBinary = 0x15a,
    ShortcutCss = 0x15b,
    ShortcutCsv = 0x15c,
    ShortcutWordDocument = 0x15d,
    ShortcutDocker = 0x15e,
    ShortcutDocuments = 0x15f,
    ShortcutDownloads = 0x160,
    ShortcutEnv = 0x161,
    ShortcutArchive = 0x162,
    ShortcutHtml = 0x163,
    ShortcutJavaScript = 0x164,
    ShortcutManifest = 0x165,
    ShortcutMarkdown = 0x166,
    ShortcutPdf = 0x167,
    ShortcutPresentation = 0x168,
    ShortcutProgram = 0x169,
    ShortcutSecret = 0x16a,
    ShortcutText = 0x16b,
    ShortcutVideo = 0x16c,
    ShortcutSpreadsheet = 0x16d,
    ShortcutYaml = 0x16e,
    ShortcutPictures = 0x16f,
    ShortcutNetworkMounts = 0x170,
}

impl IconId {
    #[must_use]
    pub const fn wire_value(self) -> u16 {
        self as u16
    }

    #[must_use]
    pub const fn from_wire(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Application),
            2 => Some(Self::Document),
            3 => Some(Self::Folder),
            4 => Some(Self::FolderOpen),
            5 => Some(Self::Home),
            6 => Some(Self::Trash),
            10 => Some(Self::DocumentNew),
            11 => Some(Self::DocumentOpen),
            12 => Some(Self::DocumentSave),
            20 => Some(Self::EditUndo),
            21 => Some(Self::EditRedo),
            22 => Some(Self::EditCut),
            23 => Some(Self::EditCopy),
            24 => Some(Self::EditPaste),
            25 => Some(Self::EditDelete),
            30 => Some(Self::Search),
            31 => Some(Self::GoBack),
            32 => Some(Self::GoForward),
            33 => Some(Self::GoUp),
            34 => Some(Self::Refresh),
            40 => Some(Self::ViewList),
            41 => Some(Self::ViewGrid),
            50 => Some(Self::Settings),
            51 => Some(Self::Information),
            52 => Some(Self::Warning),
            53 => Some(Self::Error),
            54 => Some(Self::Close),
            55 => Some(Self::Minimize),
            56 => Some(Self::Maximize),
            57 => Some(Self::Restore),
            58 => Some(Self::TrashFull),
            60 => Some(Self::Calculator),
            61 => Some(Self::TextEditor),
            62 => Some(Self::Files),
            63 => Some(Self::ImageViewer),
            64 => Some(Self::Drive),
            65 => Some(Self::Network),
            66 => Some(Self::Lock),
            67 => Some(Self::DocumentSaveAs),
            68 => Some(Self::EditFind),
            69 => Some(Self::EditRename),
            70 => Some(Self::Desktop),
            71 => Some(Self::Image),
            72 => Some(Self::Terminal),
            73 => Some(Self::ChevronUp),
            74 => Some(Self::ChevronDown),
            75 => Some(Self::Menu),
            76 => Some(Self::ObjectAdd),
            77 => Some(Self::ObjectRemove),
            78 => Some(Self::SelectionCheck),
            79 => Some(Self::ViewDetail),
            80 => Some(Self::Link),
            81 => Some(Self::MediaSeekBack),
            82 => Some(Self::MediaPlay),
            83 => Some(Self::MediaPause),
            84 => Some(Self::MediaSeekForward),
            85 => Some(Self::AudioVolume),
            86 => Some(Self::AudioMuted),
            87 => Some(Self::Fullscreen),
            88 => Some(Self::Calendar),
            89 => Some(Self::Audio),
            90 => Some(Self::Binary),
            91 => Some(Self::Css),
            92 => Some(Self::Csv),
            93 => Some(Self::WordDocument),
            94 => Some(Self::Docker),
            95 => Some(Self::Documents),
            96 => Some(Self::Downloads),
            97 => Some(Self::Env),
            98 => Some(Self::Archive),
            99 => Some(Self::Html),
            100 => Some(Self::JavaScript),
            101 => Some(Self::Manifest),
            102 => Some(Self::Markdown),
            103 => Some(Self::Pdf),
            104 => Some(Self::Presentation),
            105 => Some(Self::Program),
            106 => Some(Self::Secret),
            107 => Some(Self::Text),
            108 => Some(Self::Video),
            109 => Some(Self::Spreadsheet),
            110 => Some(Self::Yaml),
            111 => Some(Self::Pictures),
            112 => Some(Self::NetworkMounts),
            113 => Some(Self::RicewmLogo),
            114 => Some(Self::WelcomeEveryday),
            115 => Some(Self::WelcomeCreate),
            116 => Some(Self::WelcomeConnect),
            117 => Some(Self::WelcomeExtensions),
            118 => Some(Self::WelcomePackages),
            119 => Some(Self::WelcomePrivacy),
            120 => Some(Self::WelcomeHeroColor),
            121 => Some(Self::Bowl),
            122 => Some(Self::WelcomeSpeed),
            123 => Some(Self::WelcomeDesktop),
            124 => Some(Self::WelcomePrivate),
            125 => Some(Self::WelcomeBrand),
            126 => Some(Self::FormatBold),
            127 => Some(Self::FormatItalic),
            128 => Some(Self::FormatUnderline),
            129 => Some(Self::FormatStrike),
            130 => Some(Self::MailUnread),
            131 => Some(Self::MailRead),
            132 => Some(Self::Grain),
            133 => Some(Self::Star),
            600 => Some(Self::Sun),
            601 => Some(Self::StarOutline),
            602 => Some(Self::Briefcase),
            134 => Some(Self::SubmenuRight),
            135 => Some(Self::SubmenuLeft),
            136 => Some(Self::WindowsLogo),
            137 => Some(Self::Printer),
            138 => Some(Self::Inbox),
            139 => Some(Self::User),
            140 => Some(Self::Comment),
            141 => Some(Self::LinuxLogo),
            142 => Some(Self::SnapLogo),
            143 => Some(Self::Power),
            144 => Some(Self::Logout),
            145 => Some(Self::Reboot),
            147 => Some(Self::Camera),
            148 => Some(Self::Attachment),
            149 => Some(Self::MailReply),
            150 => Some(Self::MailForward),
            151 => Some(Self::FormatAlignLeft),
            152 => Some(Self::FormatAlignCenter),
            153 => Some(Self::FormatAlignRight),
            154 => Some(Self::FormatAlignJustify),
            155 => Some(Self::FormatBullets),
            156 => Some(Self::FormatNumbering),
            157 => Some(Self::FormatFontSmaller),
            158 => Some(Self::FormatFontLarger),
            159 => Some(Self::GoogleDrive),
            160 => Some(Self::Nextcloud),
            161 => Some(Self::S3Storage),
            162 => Some(Self::DataFilter),
            163 => Some(Self::DataFilterClear),
            164 => Some(Self::SortAscending),
            165 => Some(Self::SortDescending),
            166 => Some(Self::FormulaFunction),
            167 => Some(Self::FormulaSum),
            168 => Some(Self::FormulaLookup),
            169 => Some(Self::FillDown),
            170 => Some(Self::FillRight),
            171 => Some(Self::PasteValues),
            172 => Some(Self::PasteFormulas),
            173 => Some(Self::RowHeight),
            174 => Some(Self::ColumnWidth),
            175 => Some(Self::FormatNumber),
            176 => Some(Self::FormatPercent),
            177 => Some(Self::FormatCurrency),
            178 => Some(Self::InsertRows),
            179 => Some(Self::InsertColumns),
            180 => Some(Self::DeleteRows),
            181 => Some(Self::DeleteColumns),
            182 => Some(Self::Shapes),
            183 => Some(Self::Line),
            184 => Some(Self::Rectangle),
            185 => Some(Self::Ellipse),
            186 => Some(Self::FillColor),
            187 => Some(Self::OutlineColor),
            188 => Some(Self::TextColor),
            189 => Some(Self::Arrange),
            190 => Some(Self::AlignObjects),
            191 => Some(Self::DistributeHorizontal),
            192 => Some(Self::DistributeVertical),
            193 => Some(Self::Crop),
            194 => Some(Self::TextBox),
            195 => Some(Self::Rotate),
            196 => Some(Self::BringForward),
            197 => Some(Self::SendBackward),
            198 => Some(Self::ShapeRectangle),
            199 => Some(Self::ShapeRoundedRectangle),
            200 => Some(Self::ShapeEllipse),
            201 => Some(Self::ShapeTriangle),
            202 => Some(Self::ShapeRightTriangle),
            203 => Some(Self::ShapeDiamond),
            204 => Some(Self::ShapeParallelogram),
            205 => Some(Self::ShapeTrapezoid),
            206 => Some(Self::ShapeCross),
            207 => Some(Self::ShapeHeart),
            208 => Some(Self::ShapePentagon),
            209 => Some(Self::ShapeHexagon),
            210 => Some(Self::ShapeHeptagon),
            211 => Some(Self::ShapeOctagon),
            212 => Some(Self::ShapeDecagon),
            213 => Some(Self::ShapeDodecagon),
            214 => Some(Self::ShapeRightArrow),
            215 => Some(Self::ShapeDownArrow),
            216 => Some(Self::ShapeLeftArrow),
            217 => Some(Self::ShapeUpArrow),
            218 => Some(Self::ShapeLeftRightArrow),
            219 => Some(Self::ShapeUpDownArrow),
            220 => Some(Self::ShapeChevron),
            221 => Some(Self::ShapeHomePlate),
            222 => Some(Self::ShapeStar4),
            223 => Some(Self::ShapeStar5),
            224 => Some(Self::ShapeStar6),
            225 => Some(Self::ShapeStar8),
            226 => Some(Self::ShapeStar10),
            227 => Some(Self::ShapeStar12),
            228 => Some(Self::ShapeSpeechCallout),
            229 => Some(Self::ShapeDocument),
            230 => Some(Self::ObjectAlignLeft),
            231 => Some(Self::ObjectAlignCenter),
            232 => Some(Self::ObjectAlignRight),
            233 => Some(Self::ObjectAlignTop),
            234 => Some(Self::ObjectAlignMiddle),
            235 => Some(Self::ObjectAlignBottom),
            236 => Some(Self::CellStyles),
            237 => Some(Self::CellBorders),
            238 => Some(Self::FormatCells),
            239 => Some(Self::ObjectGroup),
            240 => Some(Self::ObjectUngroup),
            241 => Some(Self::ChartColumn),
            242 => Some(Self::ChartLine),
            243 => Some(Self::ChartFormat),
            244 => Some(Self::ChartData),
            245 => Some(Self::ChartBar),
            246 => Some(Self::ChartArea),
            247 => Some(Self::ChartPie),
            248 => Some(Self::ChartDoughnut),
            249 => Some(Self::ChartScatter),
            250 => Some(Self::ChartBubble),
            251 => Some(Self::ChartRadar),
            252 => Some(Self::ChartStock),
            253 => Some(Self::ChartCombo),
            254 => Some(Self::ChartSurface),
            255 => Some(Self::ChartStatistical),
            256 => Some(Self::ChartWaterfall),
            512 => Some(Self::ChartFunnel),
            513 => Some(Self::ChartHierarchy),
            514 => Some(Self::FormatPainter),
            515 => Some(Self::FormatLineSpacing),
            516 => Some(Self::ZoomIn),
            517 => Some(Self::ZoomOut),
            518 => Some(Self::DrawPolygon),
            519 => Some(Self::DrawPolyline),
            520 => Some(Self::ShapeSnip1Rect),
            521 => Some(Self::ShapeSnip2SameRect),
            522 => Some(Self::ShapeSnip2DiagRect),
            523 => Some(Self::ShapeRound1Rect),
            524 => Some(Self::ShapeRound2SameRect),
            525 => Some(Self::ShapeRound2DiagRect),
            526 => Some(Self::ShapeSnipRoundRect),
            527 => Some(Self::ShapeBevel),
            528 => Some(Self::ShapeCylinder),
            529 => Some(Self::ShapeCube),
            530 => Some(Self::ShapeCorner),
            531 => Some(Self::ShapeFoldedCorner),
            532 => Some(Self::ShapeLightningBolt),
            533 => Some(Self::ShapeMoon),
            534 => Some(Self::ShapeSun),
            535 => Some(Self::ShapeTeardrop),
            536 => Some(Self::ShapeDonut),
            537 => Some(Self::ShapeQuadArrow),
            538 => Some(Self::ShapeLeftRightUpArrow),
            539 => Some(Self::ShapeBentArrow),
            540 => Some(Self::ShapeUturnArrow),
            541 => Some(Self::ShapeLeftUpArrow),
            542 => Some(Self::ShapeNotchedRightArrow),
            543 => Some(Self::ShapeMathPlus),
            544 => Some(Self::ShapeMathMinus),
            545 => Some(Self::ShapeMathMultiply),
            546 => Some(Self::ShapeMathDivide),
            547 => Some(Self::ShapeMathEqual),
            548 => Some(Self::ShapeMathNotEqual),
            549 => Some(Self::ShapeFlowChartPredefinedProcess),
            550 => Some(Self::ShapeFlowChartInternalStorage),
            551 => Some(Self::ShapeFlowChartTerminator),
            552 => Some(Self::ShapeFlowChartPreparation),
            553 => Some(Self::ShapeFlowChartManualInput),
            554 => Some(Self::ShapeFlowChartManualOperation),
            555 => Some(Self::ShapeFlowChartOffpageConnector),
            556 => Some(Self::ShapeFlowChartDelay),
            557 => Some(Self::ShapeFlowChartDisplay),
            558 => Some(Self::ShapeStar16),
            559 => Some(Self::ShapeStar24),
            560 => Some(Self::ShapeStar32),
            561 => Some(Self::ShapeRoundCallout),
            562 => Some(Self::ShapeEllipseCallout),
            563 => Some(Self::SpreadsheetFile),
            564 => Some(Self::PresentationFile),
            565 => Some(Self::FormatWrapText),
            566 => Some(Self::FormatDecimalIncrease),
            567 => Some(Self::FormatDecimalDecrease),
            568 => Some(Self::DataRemoveDuplicates),
            569 => Some(Self::DataTextToColumns),
            570 => Some(Self::ShapeShadow),
            571 => Some(Self::Accessibility),
            572 => Some(Self::Appearance),
            573 => Some(Self::Taskbar),
            574 => Some(Self::Extensions),
            575 => Some(Self::Computer),
            576 => Some(Self::Brain),
            577 => Some(Self::Clock),

            0x333 => Some(Self::ShortcutSpreadsheetFile),
            0x334 => Some(Self::ShortcutPresentationFile),
            0x101 => Some(Self::ShortcutApplication),
            0x102 => Some(Self::ShortcutDocument),
            0x103 => Some(Self::ShortcutFolder),
            0x105 => Some(Self::ShortcutHome),
            0x146 => Some(Self::ShortcutDesktop),
            0x147 => Some(Self::ShortcutImage),
            0x159 => Some(Self::ShortcutAudio),
            0x15a => Some(Self::ShortcutBinary),
            0x15b => Some(Self::ShortcutCss),
            0x15c => Some(Self::ShortcutCsv),
            0x15d => Some(Self::ShortcutWordDocument),
            0x15e => Some(Self::ShortcutDocker),
            0x15f => Some(Self::ShortcutDocuments),
            0x160 => Some(Self::ShortcutDownloads),
            0x161 => Some(Self::ShortcutEnv),
            0x162 => Some(Self::ShortcutArchive),
            0x163 => Some(Self::ShortcutHtml),
            0x164 => Some(Self::ShortcutJavaScript),
            0x165 => Some(Self::ShortcutManifest),
            0x166 => Some(Self::ShortcutMarkdown),
            0x167 => Some(Self::ShortcutPdf),
            0x168 => Some(Self::ShortcutPresentation),
            0x169 => Some(Self::ShortcutProgram),
            0x16a => Some(Self::ShortcutSecret),
            0x16b => Some(Self::ShortcutText),
            0x16c => Some(Self::ShortcutVideo),
            0x16d => Some(Self::ShortcutSpreadsheet),
            0x16e => Some(Self::ShortcutYaml),
            0x16f => Some(Self::ShortcutPictures),
            0x170 => Some(Self::ShortcutNetworkMounts),
            _ => None,
        }
    }

    /// Wrap a semantic target icon in a shortcut badge without discarding the
    /// target identity supplied by the active theme.
    #[must_use]
    pub const fn shortcut(self) -> Self {
        match self {
            Self::SpreadsheetFile => Self::ShortcutSpreadsheetFile,
            Self::PresentationFile => Self::ShortcutPresentationFile,
            Self::Application => Self::ShortcutApplication,
            Self::Folder | Self::FolderOpen => Self::ShortcutFolder,
            Self::Home => Self::ShortcutHome,
            Self::Desktop => Self::ShortcutDesktop,
            Self::Image | Self::ImageViewer => Self::ShortcutImage,
            Self::Audio => Self::ShortcutAudio,
            Self::Binary => Self::ShortcutBinary,
            Self::Css => Self::ShortcutCss,
            Self::Csv => Self::ShortcutCsv,
            Self::WordDocument => Self::ShortcutWordDocument,
            Self::Docker => Self::ShortcutDocker,
            Self::Documents => Self::ShortcutDocuments,
            Self::Downloads => Self::ShortcutDownloads,
            Self::Env => Self::ShortcutEnv,
            Self::Archive => Self::ShortcutArchive,
            Self::Html => Self::ShortcutHtml,
            Self::JavaScript => Self::ShortcutJavaScript,
            Self::Manifest => Self::ShortcutManifest,
            Self::Markdown => Self::ShortcutMarkdown,
            Self::Pdf => Self::ShortcutPdf,
            Self::Presentation => Self::ShortcutPresentation,
            Self::Program => Self::ShortcutProgram,
            Self::Secret => Self::ShortcutSecret,
            Self::Text | Self::TextEditor => Self::ShortcutText,
            Self::Video | Self::MediaPlay => Self::ShortcutVideo,
            Self::Spreadsheet => Self::ShortcutSpreadsheet,
            Self::Yaml => Self::ShortcutYaml,
            Self::Pictures => Self::ShortcutPictures,
            Self::NetworkMounts => Self::ShortcutNetworkMounts,
            _ => Self::ShortcutDocument,
        }
    }

    /// Return the target icon and whether this semantic value is a shortcut.
    #[must_use]
    pub const fn shortcut_base(self) -> (Self, bool) {
        let base = match self {
            Self::ShortcutSpreadsheetFile => Self::SpreadsheetFile,
            Self::ShortcutPresentationFile => Self::PresentationFile,
            Self::ShortcutApplication => Self::Application,
            Self::ShortcutDocument => Self::Document,
            Self::ShortcutFolder => Self::Folder,
            Self::ShortcutHome => Self::Home,
            Self::ShortcutDesktop => Self::Desktop,
            Self::ShortcutImage => Self::Image,
            Self::ShortcutAudio => Self::Audio,
            Self::ShortcutBinary => Self::Binary,
            Self::ShortcutCss => Self::Css,
            Self::ShortcutCsv => Self::Csv,
            Self::ShortcutWordDocument => Self::WordDocument,
            Self::ShortcutDocker => Self::Docker,
            Self::ShortcutDocuments => Self::Documents,
            Self::ShortcutDownloads => Self::Downloads,
            Self::ShortcutEnv => Self::Env,
            Self::ShortcutArchive => Self::Archive,
            Self::ShortcutHtml => Self::Html,
            Self::ShortcutJavaScript => Self::JavaScript,
            Self::ShortcutManifest => Self::Manifest,
            Self::ShortcutMarkdown => Self::Markdown,
            Self::ShortcutPdf => Self::Pdf,
            Self::ShortcutPresentation => Self::Presentation,
            Self::ShortcutProgram => Self::Program,
            Self::ShortcutSecret => Self::Secret,
            Self::ShortcutText => Self::Text,
            Self::ShortcutVideo => Self::Video,
            Self::ShortcutSpreadsheet => Self::Spreadsheet,
            Self::ShortcutYaml => Self::Yaml,
            Self::ShortcutPictures => Self::Pictures,
            Self::ShortcutNetworkMounts => Self::NetworkMounts,
            _ => return (self, false),
        };
        (base, true)
    }
}

/// Selects the semantic icon for a well-known directory without coupling file
/// views to a particular theme glyph. Themes remain free to map the returned
/// semantic key to a monochrome, colored, or entirely different glyph.
#[must_use]
pub fn folder_icon(path_or_name: &str) -> IconId {
    let trimmed = path_or_name.trim_end_matches(['/', '\\']);
    if trimmed == "/mnt/local"
        || trimmed.starts_with("/mnt/local/")
        || trimmed == "/run/ricewm/mounts"
        || trimmed.starts_with("/run/ricewm/mounts/")
    {
        return IconId::NetworkMounts;
    }
    let name = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    if trimmed == "/root" || name.eq_ignore_ascii_case("home") {
        IconId::Home
    } else if name.eq_ignore_ascii_case("desktop") {
        IconId::Desktop
    } else if name.eq_ignore_ascii_case("documents") {
        IconId::Documents
    } else if name.eq_ignore_ascii_case("downloads") {
        IconId::Downloads
    } else if name.eq_ignore_ascii_case("pictures") || name.eq_ignore_ascii_case("photos") {
        IconId::Pictures
    } else {
        IconId::Folder
    }
}

/// Selects a semantic file icon from a bounded MIME hint and filename.
/// Filename recognition is intentional: browser mounts often supply the
/// generic `application/octet-stream` type, and dotfiles such as `.env` have
/// no conventional extension.
#[must_use]
pub fn file_icon(name: &str, mime_type: Option<&str>) -> IconId {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let folded = basename.to_ascii_lowercase();
    let extension = folded.rsplit_once('.').map(|(_, value)| value);
    let mime = mime_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if matches!(folded.as_str(), "dockerfile" | "containerfile")
        || extension == Some("dockerfile")
        || mime == "text/x-dockerfile"
    {
        return IconId::Docker;
    }
    if folded == ".env" || folded.starts_with(".env.") || mime == "text/x-env" {
        return IconId::Env;
    }
    if folded == "manifest.json"
        || folded.ends_with(".manifest")
        || extension == Some("webmanifest")
        || mime == "application/manifest+json"
    {
        return IconId::Manifest;
    }
    if matches!(
        extension,
        Some("pem" | "key" | "p12" | "pfx" | "gpg" | "asc")
    ) || matches!(
        mime.as_str(),
        "application/pkcs8"
            | "application/pkcs12"
            | "application/pgp-keys"
            | "application/x-pem-file"
    ) {
        return IconId::Secret;
    }
    if matches!(
        folded.as_str(),
        "license" | "licence" | "copying" | "notice" | "authors" | "changelog"
    ) || matches!(
        folded.as_str(),
        ".gitignore" | ".gitattributes" | ".dockerignore" | ".npmignore" | ".editorconfig"
    ) {
        return IconId::Text;
    }

    match extension {
        Some("mp3" | "wav" | "flac" | "m4a" | "aac" | "oga" | "opus") => IconId::Audio,
        Some("bin" | "o" | "obj" | "so" | "dll" | "dylib" | "class" | "wasm") => IconId::Binary,
        Some("css" | "scss" | "sass" | "less") => IconId::Css,
        Some("csv" | "tsv") => IconId::Csv,
        Some("doc" | "docx" | "odt" | "rtf") => IconId::WordDocument,
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" | "zst") => IconId::Archive,
        Some("html" | "htm" | "xhtml") => IconId::Html,
        Some("js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx") => IconId::JavaScript,
        Some("json") => IconId::Manifest,
        Some("md" | "markdown" | "mdown" | "mkd") => IconId::Markdown,
        Some("pdf") => IconId::Pdf,
        Some("ppt" | "pptx" | "potx" | "ppsx" | "odp" | "otp") => IconId::PresentationFile,
        Some("sh" | "bash" | "zsh" | "fish" | "exe" | "appimage" | "deb" | "rpm") => {
            IconId::Program
        }
        Some("txt" | "log" | "nfo") => IconId::Text,
        Some("toml" | "xml" | "rs" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp") => IconId::Text,
        Some(
            "mp4" | "m4v" | "webm" | "mkv" | "ogv" | "mov" | "move" | "wmv" | "asf" | "avi"
            | "mpeg" | "mpg" | "m2ts",
        ) => IconId::Video,
        Some("xls" | "xlsx" | "ods") => IconId::SpreadsheetFile,
        Some("yaml" | "yml") => IconId::Yaml,
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" | "svg") => IconId::Image,
        _ if mime.starts_with("audio/") => IconId::Audio,
        _ if mime.starts_with("video/") => IconId::Video,
        _ if mime.starts_with("image/") => IconId::Image,
        _ if matches!(mime.as_str(), "text/css") => IconId::Css,
        _ if matches!(mime.as_str(), "text/csv" | "text/tab-separated-values") => IconId::Csv,
        _ if matches!(mime.as_str(), "text/html" | "application/xhtml+xml") => IconId::Html,
        _ if matches!(
            mime.as_str(),
            "application/javascript"
                | "text/javascript"
                | "application/typescript"
                | "text/typescript"
        ) =>
        {
            IconId::JavaScript
        }
        _ if matches!(mime.as_str(), "text/markdown") => IconId::Markdown,
        _ if matches!(
            mime.as_str(),
            "application/json" | "application/manifest+json"
        ) =>
        {
            IconId::Manifest
        }
        _ if matches!(mime.as_str(), "application/toml" | "application/xml") => IconId::Text,
        _ if matches!(mime.as_str(), "application/pdf") => IconId::Pdf,
        _ if matches!(
            mime.as_str(),
            "application/zip"
                | "application/x-7z-compressed"
                | "application/x-rar-compressed"
                | "application/x-tar"
                | "application/gzip"
                | "application/x-bzip2"
                | "application/x-xz"
        ) =>
        {
            IconId::Archive
        }
        _ if matches!(
            mime.as_str(),
            "application/msword"
                | "application/rtf"
                | "application/vnd.oasis.opendocument.text"
                | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        ) =>
        {
            IconId::WordDocument
        }
        _ if matches!(
            mime.as_str(),
            "application/vnd.ms-powerpoint"
                | "application/vnd.oasis.opendocument.presentation"
                | "application/vnd.oasis.opendocument.presentation-template"
                | "application/vnd.openxmlformats-officedocument.presentationml.template"
                | "application/vnd.openxmlformats-officedocument.presentationml.slideshow"
                | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        ) =>
        {
            IconId::PresentationFile
        }
        _ if matches!(
            mime.as_str(),
            "application/vnd.ms-excel"
                | "application/vnd.oasis.opendocument.spreadsheet"
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        ) =>
        {
            IconId::SpreadsheetFile
        }
        _ if matches!(
            mime.as_str(),
            "application/yaml" | "text/yaml" | "application/x-yaml"
        ) =>
        {
            IconId::Yaml
        }
        _ if matches!(
            mime.as_str(),
            "application/wasm"
                | "application/x-object"
                | "application/x-sharedlib"
                | "application/java-vm"
        ) =>
        {
            IconId::Binary
        }
        _ if matches!(
            mime.as_str(),
            "text/x-shellscript" | "application/x-executable" | "application/x-appimage"
        ) =>
        {
            IconId::Program
        }
        _ if mime.starts_with("text/") => IconId::Text,
        _ if mime == "application/octet-stream" => IconId::Binary,
        _ => IconId::Document,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum WidgetKind {
    Container = 1,
    Row = 2,
    Column = 3,
    Stack = 4,
    SplitPane = 5,
    Label = 10,
    Image = 11,
    Button = 12,
    Toggle = 13,
    CheckBox = 14,
    RadioButton = 15,
    ProgressBar = 16,
    Spinner = 17,
    TextField = 20,
    TextEditor = 21,
    ScrollView = 22,
    ScrollBar = 23,
    PasswordField = 24,
    /// Read-only, theme-rendered Markdown content.
    Markdown = 25,
    MenuBar = 30,
    Menu = 31,
    MenuItem = 32,
    ToolBar = 33,
    Tabs = 34,
    Breadcrumb = 35,
    Slider = 36,
    List = 40,
    Table = 41,
    Tree = 42,
    IconGrid = 43,
    /// One semantic node in a [`WidgetKind::Tree`] hierarchy.
    ///
    /// Child `TreeItem`s establish depth. `CHECKED` means expanded for a
    /// container node; `SELECTED` remains independent row selection.
    TreeItem = 44,
    /// One bounded virtual spreadsheet viewport; cells are presentation data.
    VirtualGrid = 45,
    Dialog = 50,
    Popover = 51,
    Tooltip = 52,
    Notification = 53,
    /// Expandable disclosure panel. `CHECKED` exposes its child content.
    Accordion = 54,
    /// A labeled, non-interactive frame around a related set of controls.
    ///
    /// Group boxes communicate relationship only. They never receive focus,
    /// selection, pressed, or disabled presentation of their own.
    GroupBox = 55,
    /// A retained scene produced by an in-Wasm web engine.
    WebSurface = 60,
    /// A live xterm session composited into a native RiceWM window.
    TerminalSurface = 61,
    /// A capability-isolated host-rendered client surface.
    HostSurface = 62,
    TerminalGrid = 63,
    TerminalRow = 64,
    /// An owned, bounded vector scene without browser or media semantics.
    SceneSurface = 65,
}

impl WidgetKind {
    #[must_use]
    pub const fn from_wire(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Container),
            2 => Some(Self::Row),
            3 => Some(Self::Column),
            4 => Some(Self::Stack),
            5 => Some(Self::SplitPane),
            10 => Some(Self::Label),
            11 => Some(Self::Image),
            12 => Some(Self::Button),
            13 => Some(Self::Toggle),
            14 => Some(Self::CheckBox),
            15 => Some(Self::RadioButton),
            16 => Some(Self::ProgressBar),
            17 => Some(Self::Spinner),
            20 => Some(Self::TextField),
            21 => Some(Self::TextEditor),
            22 => Some(Self::ScrollView),
            23 => Some(Self::ScrollBar),
            24 => Some(Self::PasswordField),
            25 => Some(Self::Markdown),
            30 => Some(Self::MenuBar),
            31 => Some(Self::Menu),
            32 => Some(Self::MenuItem),
            33 => Some(Self::ToolBar),
            34 => Some(Self::Tabs),
            35 => Some(Self::Breadcrumb),
            36 => Some(Self::Slider),
            40 => Some(Self::List),
            41 => Some(Self::Table),
            42 => Some(Self::Tree),
            43 => Some(Self::IconGrid),
            44 => Some(Self::TreeItem),
            45 => Some(Self::VirtualGrid),
            50 => Some(Self::Dialog),
            51 => Some(Self::Popover),
            52 => Some(Self::Tooltip),
            53 => Some(Self::Notification),
            54 => Some(Self::Accordion),
            55 => Some(Self::GroupBox),
            60 => Some(Self::WebSurface),
            61 => Some(Self::TerminalSurface),
            62 => Some(Self::HostSurface),
            63 => Some(Self::TerminalGrid),
            64 => Some(Self::TerminalRow),
            65 => Some(Self::SceneSurface),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WidgetState(u32);

impl WidgetState {
    pub const ENABLED: Self = Self(1 << 0);
    pub const FOCUSED: Self = Self(1 << 1);
    pub const HOVERED: Self = Self(1 << 2);
    pub const PRESSED: Self = Self(1 << 3);
    pub const SELECTED: Self = Self(1 << 4);
    pub const CHECKED: Self = Self(1 << 5);
    pub const INDETERMINATE: Self = Self(1 << 6);
    pub const INVALID: Self = Self(1 << 7);
    pub const BUSY: Self = Self(1 << 8);
    /// Visually identifies the primary action in a group.
    pub const ACCENT: Self = Self(1 << 9);
    /// Visually identifies an action that can discard or destroy data.
    pub const DESTRUCTIVE: Self = Self(1 << 10);
    /// Requests stronger typographic hierarchy without changing the content.
    pub const EMPHASIZED: Self = Self(1 << 11);
    /// Requests secondary, de-emphasized text.
    pub const MUTED: Self = Self(1 << 12);
    /// Aligns textual content to the logical end of its available rectangle.
    pub const END_ALIGNED: Self = Self(1 << 13);
    /// Keeps an interactive control functional while removing its raised chrome.
    pub const FLAT: Self = Self(1 << 14);
    /// Requests prominent numeric/readout typography.
    pub const DISPLAY: Self = Self(1 << 15);
    /// Aligns textual content to the logical start of its available rectangle.
    ///
    /// Interactive controls default to centered labels, while navigation rows
    /// and table headings use this flag for conventional list alignment.
    pub const START_ALIGNED: Self = Self(1 << 16);
    /// Leaves Enter/Space available to the owning collection for conventional
    /// keyboard navigation instead of treating both keys as button clicks.
    pub const KEY_NAVIGATION: Self = Self(1 << 17);
    /// Selects the complete value when a single-line text control receives
    /// primary pointer focus. Typing replaces it; a navigation key first
    /// collapses the selection without changing the value.
    pub const SELECT_ALL_ON_FOCUS: Self = Self(1 << 18);
    /// Places a semantic icon at the logical end of the control.
    ///
    /// Navigation rows and choice controls use this for conventional trailing
    /// carets without application-owned spacing or glyphs.
    pub const TRAILING_ICON: Self = Self(1 << 19);
    /// Requests a workspace-sized modal for browsing or managing substantial content.
    ///
    /// File choosers and similarly dense pickers use this role; confirmation and
    /// preference dialogs remain content-sized.
    pub const EXPANSIVE: Self = Self(1 << 20);
    /// Centers non-interactive copy such as onboarding heroes and empty states.
    pub const CENTER_ALIGNED: Self = Self(1 << 21);
    /// Requests a large application-identity icon without changing ordinary images.
    pub const LARGE_ICON: Self = Self(1 << 22);
    /// Uses the theme's informational surface role.
    pub const INFORMATION: Self = Self(1 << 23);
    /// Uses the theme's positive surface role.
    pub const SUCCESS: Self = Self(1 << 24);
    /// Uses the theme's warm discovery surface role.
    pub const WARNING: Self = Self(1 << 25);
    /// Requests restrained ambient motion when the desktop motion setting allows it.
    pub const AMBIENT_MOTION: Self = Self(1 << 26);
    /// Renders the control as refractive liquid glass over preceding content.
    pub const LIQUID_GLASS: Self = Self(1 << 27);
    /// Renders a theme-independent scenic dark backdrop beneath child content.
    pub const SCENIC_BACKDROP: Self = Self(1 << 28);
    /// Selects the bundled Manrope brand face for display and identity copy.
    pub const BRAND_TYPE: Self = Self(1 << 29);
    /// Paints brand copy through the product accent gradient.
    pub const ACCENT_GRADIENT: Self = Self(1 << 30);
    /// Requests hero-scale typography while retaining responsive layout.
    pub const PROMINENT: Self = Self(1 << 31);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn from_bits_retain(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for WidgetState {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Widget {
    pub id: WidgetId,
    pub kind: WidgetKind,
    pub state: WidgetState,
    pub text: Option<String>,
    pub icon: Option<IconId>,
    pub parent: Option<WidgetId>,
    pub(crate) children: Vec<WidgetId>,
}

impl Widget {
    #[must_use]
    pub fn children(&self) -> &[WidgetId] {
        &self.children
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WidgetSpec {
    pub id: WidgetId,
    pub kind: WidgetKind,
    pub state: WidgetState,
    pub text: Option<String>,
    pub icon: Option<IconId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueChange<T> {
    Keep,
    Set(T),
    Clear,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Patch {
    Create {
        widget: WidgetSpec,
        parent: Option<WidgetId>,
        index: usize,
    },
    Update {
        id: WidgetId,
        state: Option<WidgetState>,
        text: ValueChange<String>,
        icon: ValueChange<IconId>,
    },
    Move {
        id: WidgetId,
        parent: Option<WidgetId>,
        index: usize,
    },
    Remove {
        id: WidgetId,
    },
    Clear,
    GridDamage { id: WidgetId, damage: String },
    ObserveSize { id: WidgetId, enabled: bool },
    EditSelection { id: WidgetId, anchor: u32, caret: u32 },
    DocumentFont { id: WidgetId, font: Option<String> },
    /// Stable theme override selector such as app:native:rice-files or mime:application/pdf.
    IconSelector { id: WidgetId, selector: Option<String> },
}
