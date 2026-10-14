//! Browser-backed mount catalog values exposed to native applications.

pub const MAX_MOUNTS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MountState {
    Connected = 0,
    PermissionRequired = 1,
    Syncing = 2,
    Error = 3,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MountCapabilities(u8);

impl MountCapabilities {
    pub const OPEN: Self = Self(1 << 0);
    pub const SYNC: Self = Self(1 << 1);
    pub const DISCONNECT: Self = Self(1 << 2);
    pub const RECONNECT: Self = Self(1 << 3);
    pub const ALL_BITS: u8 = Self::OPEN.0 | Self::SYNC.0 | Self::DISCONNECT.0 | Self::RECONNECT.0;

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits & !Self::ALL_BITS == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }
}

impl std::ops::BitOr for MountCapabilities {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MountEntry {
    pub id: String,
    pub label: String,
    pub path: String,
    pub state: MountState,
    pub capabilities: MountCapabilities,
    pub icon: Option<u16>,
}
