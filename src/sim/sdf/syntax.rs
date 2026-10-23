//! Owned typed SDF records, as the parser produces them (OVI SDF 3.0
//! chapter 4, plus the OVI SDF 2.1 forms that 3.0 removed).
//!
//! Names are unescaped: `a\+b` is the name `a+b`. Values keep their exact
//! decimal text and min:typ:max members; nothing is selected or scaled
//! here.

use super::expr::{CondExpr, SdfBits};
use super::number::SdfValue;
use super::SdfPos;
use std::fmt;

/// The grammar a file is read with, from its SDFVERSION entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfVersion {
    /// OVI SDF 2.1 (February 1994).
    V2_1,
    /// OVI SDF 3.0 (May 1995).
    V3_0,
}

impl fmt::Display for SdfVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::V2_1 => "2.1",
            Self::V3_0 => "3.0",
        })
    }
}

/// TIMESCALE units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfTimeUnit {
    Us,
    Ns,
    Ps,
}

/// The TIMESCALE entry: a multiplier of 1, 10 or 100 and a unit. Without
/// the entry, values are in nanoseconds ("This has the same effect as a
/// timescale entry of 1ns").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SdfTimescale {
    pub multiplier: u32,
    pub unit: SdfTimeUnit,
}

impl Default for SdfTimescale {
    fn default() -> Self {
        Self {
            multiplier: 1,
            unit: SdfTimeUnit::Ns,
        }
    }
}

impl SdfTimescale {
    /// One SDF time unit in femtoseconds.
    pub fn femtoseconds(&self) -> u64 {
        let unit = match self.unit {
            SdfTimeUnit::Us => 1_000_000_000,
            SdfTimeUnit::Ns => 1_000_000,
            SdfTimeUnit::Ps => 1_000,
        };
        u64::from(self.multiplier) * unit
    }
}

impl fmt::Display for SdfTimescale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = match self.unit {
            SdfTimeUnit::Us => "us",
            SdfTimeUnit::Ns => "ns",
            SdfTimeUnit::Ps => "ps",
        };
        write!(f, "{}{unit}", self.multiplier)
    }
}

/// The header section. Only the version, divider and timescale change how
/// the file is read; the other entries are documentation.
#[derive(Clone, Debug, PartialEq)]
pub struct SdfHeader {
    pub version: SdfVersion,
    /// The SDFVERSION string as written.
    pub version_text: String,
    pub design: Option<String>,
    pub date: Option<String>,
    pub vendor: Option<String>,
    pub program: Option<String>,
    pub program_version: Option<String>,
    /// `.` (the default) or `/`.
    pub divider: char,
    pub voltage: Option<SdfValue>,
    pub process: Option<String>,
    pub temperature: Option<SdfValue>,
    pub timescale: SdfTimescale,
}

/// One identifier of a hierarchical path or a port, unescaped, with its bit
/// spec.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SdfIdent {
    pub name: String,
    pub bits: Option<SdfBits>,
}

impl fmt::Display for SdfIdent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        super::expr::write_identifier(f, &self.name)?;
        if let Some(bits) = self.bits {
            write!(f, "{bits}")?;
        }
        Ok(())
    }
}

/// A port_instance: instance path components (relative to the cell's
/// region) and the port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfPortRef {
    pub pos: SdfPos,
    pub path: Vec<SdfIdent>,
    pub port: SdfIdent,
}

impl fmt::Display for SdfPortRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for component in &self.path {
            write!(f, "{component}.")?;
        }
        write!(f, "{}", self.port)
    }
}

/// EDGE_IDENTIFIER.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfEdge {
    Posedge,
    Negedge,
    E01,
    E10,
    E0z,
    Ez1,
    E1z,
    Ez0,
}

impl SdfEdge {
    pub(super) fn from_word(word: &str) -> Option<Self> {
        Some(match word.to_ascii_lowercase().as_str() {
            "posedge" => Self::Posedge,
            "negedge" => Self::Negedge,
            "01" => Self::E01,
            "10" => Self::E10,
            "0z" => Self::E0z,
            "z1" => Self::Ez1,
            "1z" => Self::E1z,
            "z0" => Self::Ez0,
            _ => return None,
        })
    }

    pub fn text(self) -> &'static str {
        match self {
            Self::Posedge => "posedge",
            Self::Negedge => "negedge",
            Self::E01 => "01",
            Self::E10 => "10",
            Self::E0z => "0z",
            Self::Ez1 => "z1",
            Self::E1z => "1z",
            Self::Ez0 => "z0",
        }
    }
}

/// A port_spec: a port with an optional edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfPortSpec {
    pub edge: Option<SdfEdge>,
    pub port: SdfPortRef,
}

impl fmt::Display for SdfPortSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.edge {
            Some(edge) => write!(f, "({} {})", edge.text(), self.port),
            None => write!(f, "{}", self.port),
        }
    }
}

/// The region a CELL entry names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdfCellInstance {
    /// A path from the annotation scope; empty for the scope itself.
    Path(Vec<SdfIdent>),
    /// `*`: every instance of the cell type within the annotation scope.
    Wildcard,
}

/// One CELL entry.
#[derive(Clone, Debug, PartialEq)]
pub struct SdfCell {
    pub pos: SdfPos,
    pub cell_type: String,
    pub instance: SdfCellInstance,
    pub specs: Vec<SdfTimingSpec>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SdfTimingSpec {
    Delay(Vec<SdfDelType>),
    TimingCheck(Vec<SdfTimingCheck>),
    /// TIMINGENV (and 2.1 constraints in TIMINGCHECK): forward annotation
    /// only. The entries are checked against the grammar and then dropped;
    /// a simulator ignores them without warnings (IEEE 1800-2009 32.3).
    TimingEnv,
}

/// The pulse limits of one delval. Most delvals have none, so the limits
/// are boxed to keep delay lists small.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdfPulse {
    /// A plain rvalue: no pulse limits are given (the annotator derives them
    /// from its pulse percentages, IEEE 1800-2009 32.7).
    Unspecified,
    /// `(delay limit)` or `(delay reject error)`.
    Limits(Box<SdfPulseLimits>),
}

/// Reject and error limits; an empty member leaves that limit unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdfPulseLimits {
    pub reject: Option<SdfValue>,
    pub error: Option<SdfValue>,
}

/// One delval: a delay with optional pulse limits; an empty delay leaves
/// the delay unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfDelval {
    pub delay: Option<SdfValue>,
    pub pulse: SdfPulse,
}

impl fmt::Display for SdfDelval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let delay = super::number::fmt_optional(self.delay.as_ref());
        match &self.pulse {
            SdfPulse::Unspecified => f.write_str(&delay),
            SdfPulse::Limits(limits) => write!(
                f,
                "({delay} {} {})",
                super::number::fmt_optional(limits.reject.as_ref()),
                super::number::fmt_optional(limits.error.as_ref())
            ),
        }
    }
}

/// A delval_list of 1 to 12 delvals. Lists of 4 or 5 mean 6 with trailing
/// empty delvals, lists of 7 to 11 mean 12 likewise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfDelvalList {
    pub values: Vec<SdfDelval>,
}

impl fmt::Display for SdfDelvalList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, value) in self.values.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{value}")?;
        }
        Ok(())
    }
}

/// A condition with its optional symbolic name (QSTRING).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfCondition {
    pub label: Option<String>,
    pub expr: CondExpr,
}

/// The condition form of an IOPATH.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdfPathCondition {
    None,
    Cond(SdfCondition),
    CondElse,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SdfDelType {
    /// PATHPULSE, or PATHPULSEPERCENT (`percent`; GLOBALPATHPULSE in 2.1).
    /// One value sets both limits.
    PathPulse {
        pos: SdfPos,
        percent: bool,
        path: Option<Box<(SdfPortRef, SdfPortRef)>>,
        reject: Option<SdfValue>,
        error: Option<SdfValue>,
    },
    Absolute(Vec<SdfDelDef>),
    Increment(Vec<SdfDelDef>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SdfDelDef {
    IoPath {
        pos: SdfPos,
        condition: SdfPathCondition,
        input: SdfPortSpec,
        output: SdfPortRef,
        retain: Vec<SdfDelvalList>,
        delays: SdfDelvalList,
    },
    Port {
        pos: SdfPos,
        port: SdfPortRef,
        delays: SdfDelvalList,
    },
    Interconnect {
        pos: SdfPos,
        source: SdfPortRef,
        load: SdfPortRef,
        delays: SdfDelvalList,
    },
    /// OVI SDF 2.1 only: a net or the output port driving it.
    NetDelay {
        pos: SdfPos,
        net: SdfPortRef,
        delays: SdfDelvalList,
    },
    Device {
        pos: SdfPos,
        port: Option<SdfPortRef>,
        delays: SdfDelvalList,
    },
}

/// The timing check entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfCheckKind {
    Setup,
    Hold,
    SetupHold,
    Recovery,
    Removal,
    RecRem,
    Skew,
    Width,
    Period,
    NoChange,
}

impl SdfCheckKind {
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Setup => "SETUP",
            Self::Hold => "HOLD",
            Self::SetupHold => "SETUPHOLD",
            Self::Recovery => "RECOVERY",
            Self::Removal => "REMOVAL",
            Self::RecRem => "RECREM",
            Self::Skew => "SKEW",
            Self::Width => "WIDTH",
            Self::Period => "PERIOD",
            Self::NoChange => "NOCHANGE",
        }
    }

    /// Ports the entry names: one for WIDTH and PERIOD, else two.
    pub fn port_count(self) -> usize {
        match self {
            Self::Width | Self::Period => 1,
            _ => 2,
        }
    }

    /// Values the entry carries: two for SETUPHOLD, RECREM and NOCHANGE.
    pub fn value_count(self) -> usize {
        match self {
            Self::SetupHold | Self::RecRem | Self::NoChange => 2,
            _ => 1,
        }
    }
}

/// A port_tchk: a port_spec with an optional COND.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfPortTchk {
    pub condition: Option<SdfCondition>,
    pub spec: SdfPortSpec,
}

/// One timing check entry: ports in SDF order (data before clock for
/// SETUP, HOLD and SETUPHOLD; the control or stamp port first otherwise)
/// and its values in SDF order.
#[derive(Clone, Debug, PartialEq)]
pub struct SdfTimingCheck {
    pub pos: SdfPos,
    pub kind: SdfCheckKind,
    pub ports: Vec<SdfPortTchk>,
    pub values: Vec<Option<SdfValue>>,
    pub scond: Option<SdfCondition>,
    pub ccond: Option<SdfCondition>,
}
