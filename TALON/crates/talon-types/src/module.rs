use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ModuleId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleId {
    Firebird,
    Thunderbird,
    Taxi,
    Carousel,
    Snapback,
    Climb,
    Sage,
    ParaShort,
    Siphon,
    YoYo,
    Payload,
}

impl ModuleId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Firebird => "firebird",
            Self::Thunderbird => "thunderbird",
            Self::Taxi => "taxi",
            Self::Carousel => "carousel",
            Self::Snapback => "snapback",
            Self::Climb => "climb",
            Self::Sage => "sage",
            Self::ParaShort => "parashort",
            Self::Siphon => "siphon",
            Self::YoYo => "yoyo",
            Self::Payload => "payload",
        }
    }

    pub fn tier(&self) -> Tier {
        match self {
            Self::Firebird | Self::Thunderbird | Self::Taxi | Self::Carousel => Tier::Hatch,
            Self::Snapback | Self::Climb => Tier::Takeoff,
            Self::Sage | Self::ParaShort | Self::Siphon | Self::YoYo | Self::Payload => {
                Tier::Payload
            }
        }
    }

    pub fn supervision_model(&self) -> SupervisionModel {
        match self {
            Self::Firebird | Self::Thunderbird | Self::Taxi | Self::Carousel => {
                SupervisionModel::SupervisedAutonomy
            }
            Self::Snapback | Self::YoYo | Self::Payload | Self::ParaShort => {
                SupervisionModel::DualControlStrict
            }
            Self::Climb | Self::Sage | Self::Siphon => SupervisionModel::DualControl,
        }
    }

    pub fn is_intraday(&self) -> bool {
        matches!(
            self,
            Self::Climb | Self::Snapback | Self::YoYo | Self::Payload
        )
    }

    pub fn is_cruising_altitude_eligible(&self) -> bool {
        matches!(
            self,
            Self::Firebird | Self::Thunderbird | Self::Taxi | Self::Siphon | Self::Carousel
        )
    }
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Tier
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tier {
    Hatch,
    Takeoff,
    Payload,
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hatch => write!(f, "Hatch"),
            Self::Takeoff => write!(f, "Takeoff"),
            Self::Payload => write!(f, "Payload"),
        }
    }
}

// ---------------------------------------------------------------------------
// Supervision model (S7.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupervisionModel {
    /// Hatch. Boundaries only. Bot executes freely within limits.
    SupervisedAutonomy,
    /// Takeoff/Payload. Per-trade approval with trust graduation.
    DualControl,
    /// Short selling, 0DTE. No auto-trust. Every trade, every time.
    DualControlStrict,
}

// ---------------------------------------------------------------------------
// Module state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleState {
    Idle,
    Scanning,
    SignalGenerated,
    PendingApproval,
    Active,
    Paused,
    Disabled,
}
