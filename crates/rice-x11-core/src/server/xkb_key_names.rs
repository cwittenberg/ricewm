//! Physical names for the evdev keycodes used by the core X11 input map.
//! These names describe key positions, independently of mutable keysyms.
//! Mapping: xkeyboard-config keycodes/evdev (xkb-data 2.41-2ubuntu1.1),
//! SHA-256 6f52c4f4e0f599a3b57913ac0c74dae4bfff1866c903eaceda45cda51a14d231.

pub(super) fn evdev_key_name(keycode: u8) -> [u8; 4] {
    keycode.checked_sub(8).map_or([0; 4], |index| EVDEV_NAMES[usize::from(index)])
}

const EVDEV_NAMES: [[u8; 4]; 248] = [
    [0; 4], *b"ESC\0", *b"AE01", *b"AE02", *b"AE03", *b"AE04", *b"AE05", *b"AE06", // 8..15
    *b"AE07", *b"AE08", *b"AE09", *b"AE10", *b"AE11", *b"AE12", *b"BKSP", *b"TAB\0", // 16..23
    *b"AD01", *b"AD02", *b"AD03", *b"AD04", *b"AD05", *b"AD06", *b"AD07", *b"AD08", // 24..31
    *b"AD09", *b"AD10", *b"AD11", *b"AD12", *b"RTRN", *b"LCTL", *b"AC01", *b"AC02", // 32..39
    *b"AC03", *b"AC04", *b"AC05", *b"AC06", *b"AC07", *b"AC08", *b"AC09", *b"AC10", // 40..47
    *b"AC11", *b"TLDE", *b"LFSH", *b"BKSL", *b"AB01", *b"AB02", *b"AB03", *b"AB04", // 48..55
    *b"AB05", *b"AB06", *b"AB07", *b"AB08", *b"AB09", *b"AB10", *b"RTSH", *b"KPMU", // 56..63
    *b"LALT", *b"SPCE", *b"CAPS", *b"FK01", *b"FK02", *b"FK03", *b"FK04", *b"FK05", // 64..71
    *b"FK06", *b"FK07", *b"FK08", *b"FK09", *b"FK10", *b"NMLK", *b"SCLK", *b"KP7\0", // 72..79
    *b"KP8\0", *b"KP9\0", *b"KPSU", *b"KP4\0", *b"KP5\0", *b"KP6\0", *b"KPAD", *b"KP1\0", // 80..87
    *b"KP2\0", *b"KP3\0", *b"KP0\0", *b"KPDL", *b"LVL3", [0; 4], *b"LSGT", *b"FK11", // 88..95
    *b"FK12", *b"AB11", *b"KATA", *b"HIRA", *b"HENK", *b"HKTG", *b"MUHE", *b"JPCM", // 96..103
    *b"KPEN", *b"RCTL", *b"KPDV", *b"PRSC", *b"RALT", *b"LNFD", *b"HOME", *b"UP\0\0", // 104..111
    *b"PGUP", *b"LEFT", *b"RGHT", *b"END\0", *b"DOWN", *b"PGDN", *b"INS\0", *b"DELE", // 112..119
    *b"I120", *b"MUTE", *b"VOL-", *b"VOL+", *b"POWR", *b"KPEQ", *b"I126", *b"PAUS", // 120..127
    *b"I128", *b"I129", *b"HNGL", *b"HJCV", *b"AE13", *b"LWIN", *b"RWIN", *b"COMP", // 128..135
    *b"STOP", *b"AGAI", *b"PROP", *b"UNDO", *b"FRNT", *b"COPY", *b"OPEN", *b"PAST", // 136..143
    *b"FIND", *b"CUT\0", *b"HELP", *b"I147", *b"I148", *b"I149", *b"I150", *b"I151", // 144..151
    *b"I152", *b"I153", *b"I154", *b"I155", *b"I156", *b"I157", *b"I158", *b"I159", // 152..159
    *b"I160", *b"I161", *b"I162", *b"I163", *b"I164", *b"I165", *b"I166", *b"I167", // 160..167
    *b"I168", *b"I169", *b"I170", *b"I171", *b"I172", *b"I173", *b"I174", *b"I175", // 168..175
    *b"I176", *b"I177", *b"I178", *b"I179", *b"I180", *b"I181", *b"I182", *b"I183", // 176..183
    *b"I184", *b"I185", *b"I186", *b"I187", *b"I188", *b"I189", *b"I190", *b"FK13", // 184..191
    *b"FK14", *b"FK15", *b"FK16", *b"FK17", *b"FK18", *b"FK19", *b"FK20", *b"FK21", // 192..199
    *b"FK22", *b"FK23", *b"FK24", *b"LVL5", *b"ALT\0", *b"META", *b"SUPR", *b"HYPR", // 200..207
    *b"I208", *b"I209", *b"I210", *b"I211", *b"I212", *b"I213", *b"I214", *b"I215", // 208..215
    *b"I216", *b"I217", *b"I218", *b"I219", *b"I220", *b"I221", *b"I222", *b"I223", // 216..223
    *b"I224", *b"I225", *b"I226", *b"I227", *b"I228", *b"I229", *b"I230", *b"I231", // 224..231
    *b"I232", *b"I233", *b"I234", *b"I235", *b"I236", *b"I237", *b"I238", *b"I239", // 232..239
    *b"I240", *b"I241", *b"I242", *b"I243", *b"I244", *b"I245", *b"I246", *b"I247", // 240..247
    *b"I248", *b"I249", *b"I250", *b"I251", *b"I252", *b"I253", *b"I254", *b"I255", // 248..255
];
