//! Valuing a resolved [`SdfPlan`] for one `$sdf_annotate` call (IEEE
//! 1800-2009 32.5-32.9, IEEE 1364-2001 16.3-16.6 and 17.2.9).
//!
//! The plan keeps the file's exact numbers. Applying it takes the call's
//! arguments: the member of each min:typ:max triple (`mtm_spec`), the scale
//! factors and the member they scale (`scale_factors`, `scale_type`), the
//! file's TIMESCALE and the time precision of each annotated module. Every
//! value is rounded once, to the precision of the module it lands in (half
//! away from zero), and stored in design-precision ticks.
//!
//! Delay lists expand to the twelve transitions of Table 32-4 for paths and
//! interconnect, and to the three gate delays of Table 28-9 (rise, fall,
//! turn-off; extra values ignored) for primitives. A transition whose source
//! values are missing (`()` or an absent triple member) keeps its value.
//! Pulse limits follow 32.7: a plain delval derives them from the pulse
//! percentages of the new delay, `(d () ())` keeps them, `(d r)` and
//! `(d r e)` give them.
//!
//! The result names annotation targets ([`SdfTargets`] declarations and
//! ports); the caller maps those to model rows.

use super::number::{SdfNumber, SdfValue};
use super::plan::{SdfDelayOp, SdfLimitSlot, SdfMode, SdfOpKind, SdfPlan, SdfPortTarget};
use super::syntax::{SdfDelvalList, SdfPulse};
use super::targets::SdfTargets;
use super::{SdfPos, SdfWarning};
use std::fmt;

/// Which member of a min:typ:max triple is annotated (Table 32-5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfMtm {
    Minimum,
    Typical,
    Maximum,
    /// The simulator's own selection (`--delay-select`).
    ToolControl,
}

impl SdfMtm {
    /// The keyword, case-insensitively.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_uppercase().as_str() {
            "MINIMUM" => Self::Minimum,
            "TYPICAL" => Self::Typical,
            "MAXIMUM" => Self::Maximum,
            "TOOL_CONTROL" => Self::ToolControl,
            _ => return None,
        })
    }

    pub fn keyword(self) -> &'static str {
        match self {
            Self::Minimum => "MINIMUM",
            Self::Typical => "TYPICAL",
            Self::Maximum => "MAXIMUM",
            Self::ToolControl => "TOOL_CONTROL",
        }
    }
}

/// Which member the scale factors apply to (Table 32-6). `FromMtm` scales
/// each member by its own factor; the others derive all three members from
/// one member, scaled by each factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfScaleType {
    FromMinimum,
    FromTypical,
    FromMaximum,
    FromMtm,
}

impl SdfScaleType {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_uppercase().as_str() {
            "FROM_MINIMUM" => Self::FromMinimum,
            "FROM_TYPICAL" => Self::FromTypical,
            "FROM_MAXIMUM" => Self::FromMaximum,
            "FROM_MTM" => Self::FromMtm,
            _ => return None,
        })
    }

    pub fn keyword(self) -> &'static str {
        match self {
            Self::FromMinimum => "FROM_MINIMUM",
            Self::FromTypical => "FROM_TYPICAL",
            Self::FromMaximum => "FROM_MAXIMUM",
            Self::FromMtm => "FROM_MTM",
        }
    }
}

/// How the interconnect delays of one load port from several sources
/// combine into its one port delay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfMipdPolicy {
    Maximum,
    Minimum,
    Average,
    /// The most recent annotation.
    Last,
}

impl SdfMipdPolicy {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_uppercase().as_str() {
            "MAXIMUM" => Self::Maximum,
            "MINIMUM" => Self::Minimum,
            "AVERAGE" => Self::Average,
            "LAST" => Self::Last,
            _ => return None,
        })
    }

    /// The runtime code, `LLG_SDF_MIPD_*`.
    pub fn code(self) -> u8 {
        match self {
            Self::Maximum => 0,
            Self::Minimum => 1,
            Self::Average => 2,
            Self::Last => 3,
        }
    }
}

/// The turn-off delay of a gate whose SDF delays give only rise and fall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SdfTurnoff {
    /// Table 28-9: the smaller of rise and fall.
    FromFile,
    Minimum,
    Maximum,
    Average,
}

impl SdfTurnoff {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_uppercase().as_str() {
            "FROM_FILE" => Self::FromFile,
            "MINIMUM" => Self::Minimum,
            "MAXIMUM" => Self::Maximum,
            "AVERAGE" => Self::Average,
            _ => return None,
        })
    }
}

/// The settings of one annotation, after the call's arguments override its
/// configuration file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdfCallOptions {
    pub mtm: SdfMtm,
    /// The member `ToolControl` selects: 0 minimum, 1 typical, 2 maximum.
    pub tool_member: usize,
    pub scale: [SdfNumber; 3],
    pub scale_type: SdfScaleType,
    pub turnoff: SdfTurnoff,
    pub mipd: SdfMipdPolicy,
}

impl Default for SdfCallOptions {
    fn default() -> Self {
        Self {
            mtm: SdfMtm::ToolControl,
            tool_member: 1,
            scale: [SdfNumber::new(1, 0); 3],
            scale_type: SdfScaleType::FromMtm,
            turnoff: SdfTurnoff::FromFile,
            mipd: SdfMipdPolicy::Maximum,
        }
    }
}

impl SdfCallOptions {
    fn member(&self) -> usize {
        match self.mtm {
            SdfMtm::Minimum => 0,
            SdfMtm::Typical => 1,
            SdfMtm::Maximum => 2,
            SdfMtm::ToolControl => self.tool_member,
        }
    }

    /// The selected, scaled member of `value`; `None` when that member is
    /// absent (the target keeps its value).
    pub fn select(&self, value: &SdfValue) -> Option<(SdfNumber, SdfNumber)> {
        let member = self.member();
        let source = match self.scale_type {
            SdfScaleType::FromMtm => member,
            SdfScaleType::FromMinimum => 0,
            SdfScaleType::FromTypical => 1,
            SdfScaleType::FromMaximum => 2,
        };
        value
            .member(source)
            .map(|number| (number, self.scale[member]))
    }
}

/// Parse `scale_factors`: three non-negative numbers `min:typ:max`.
pub fn parse_scale_factors(text: &str) -> Option<[SdfNumber; 3]> {
    let parts = text.trim().split(':').collect::<Vec<_>>();
    let [min, typ, max] = parts.as_slice() else {
        return None;
    };
    let number = |text: &str| {
        SdfNumber::parse(text.trim())
            .ok()
            .filter(|number| !number.is_negative())
    };
    Some([number(min)?, number(typ)?, number(max)?])
}

/// The settings an SDF configuration file gives; each overrides the
/// default and is overridden by the matching `$sdf_annotate` argument.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SdfConfig {
    pub mtm: Option<SdfMtm>,
    pub scale: Option<[SdfNumber; 3]>,
    pub scale_type: Option<SdfScaleType>,
    pub mipd: Option<SdfMipdPolicy>,
    pub turnoff: Option<SdfTurnoff>,
}

/// A malformed configuration file: line and message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfConfigError {
    pub line: u32,
    pub message: String,
}

impl fmt::Display for SdfConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.line, self.message)
    }
}

/// Parse an SDF configuration file: statements `KEYWORD = value ;` with
/// `//` and `/* */` comments, keywords and values in any case. MTM,
/// SCALE_FACTORS, SCALE_TYPE, INTERCONNECT_MIPD and TURNOFF_DELAY are
/// understood; a later statement replaces an earlier one. Any other
/// statement is an error, since the file controls how values are annotated
/// and a statement left out would silently change them.
pub fn parse_config(text: &str) -> Result<SdfConfig, SdfConfigError> {
    let mut config = SdfConfig::default();
    let mut statement = String::new();
    let mut statement_line = 1u32;
    let mut line = 1u32;
    let mut chars = text.chars().peekable();
    let mut finish = |statement: &mut String, at: u32| -> Result<(), SdfConfigError> {
        let text = std::mem::take(statement);
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        let error = |message: String| SdfConfigError { line: at, message };
        let Some((keyword, value)) = text.split_once('=') else {
            return Err(error(format!("`{text}` is not `KEYWORD = value`")));
        };
        let keyword = keyword.trim().to_ascii_uppercase();
        let value = value.trim();
        let bad = |what: &str| error(format!("invalid {what} value `{value}`"));
        match keyword.as_str() {
            "MTM" | "MTM_SPEC" => {
                config.mtm = Some(SdfMtm::parse(value).ok_or_else(|| bad("MTM"))?)
            }
            "SCALE_FACTORS" => {
                config.scale = Some(parse_scale_factors(value).ok_or_else(|| bad("SCALE_FACTORS"))?)
            }
            "SCALE_TYPE" => {
                config.scale_type =
                    Some(SdfScaleType::parse(value).ok_or_else(|| bad("SCALE_TYPE"))?)
            }
            "INTERCONNECT_MIPD" => {
                config.mipd =
                    Some(SdfMipdPolicy::parse(value).ok_or_else(|| bad("INTERCONNECT_MIPD"))?)
            }
            "TURNOFF_DELAY" => {
                config.turnoff = Some(SdfTurnoff::parse(value).ok_or_else(|| bad("TURNOFF_DELAY"))?)
            }
            _ => {
                return Err(error(format!(
                    "unknown statement `{keyword}`; llg reads MTM, SCALE_FACTORS, SCALE_TYPE, \
                     INTERCONNECT_MIPD and TURNOFF_DELAY"
                )))
            }
        }
        Ok(())
    };
    while let Some(character) = chars.next() {
        match character {
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        line += 1;
                        statement.push(' ');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = ' ';
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '\n' {
                        line += 1;
                    }
                    if previous == '*' && next == '/' {
                        closed = true;
                        break;
                    }
                    previous = next;
                }
                if !closed {
                    return Err(SdfConfigError {
                        line,
                        message: "unterminated comment".to_owned(),
                    });
                }
                statement.push(' ');
            }
            ';' => {
                finish(&mut statement, statement_line)?;
                statement_line = line;
            }
            '\n' => {
                line += 1;
                if statement.trim().is_empty() {
                    statement_line = line;
                }
                statement.push(' ');
            }
            _ => {
                if statement.trim().is_empty() && !character.is_whitespace() {
                    statement_line = line;
                }
                statement.push(character);
            }
        }
    }
    if !statement.trim().is_empty() {
        return Err(SdfConfigError {
            line: statement_line,
            message: format!("`{}` has no closing `;`", statement.trim()),
        });
    }
    Ok(config)
}

/// The tick scale of the model: the design precision and, per annotated
/// scope, its module's time precision.
#[derive(Clone, Copy, Debug)]
pub struct SdfTimebase {
    /// Femtoseconds per scheduler tick.
    pub design_precision_fs: u64,
}

/// Which pulse limits of a transition an op gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SdfMasks {
    /// Bit `t`: the delay of transition `t` is annotated.
    pub delay: u16,
    /// Bit `t`: the reject (error) limit of transition `t` is annotated.
    pub reject: u16,
    pub error: u16,
    /// Bit `t`: both limits derive from the pulse percentages of the new
    /// delay of transition `t` (a delval without limits, 32.7).
    pub derive: u16,
}

impl SdfMasks {
    pub fn is_empty(&self) -> bool {
        self.delay | self.reject | self.error | self.derive == 0
    }

    /// Values the op stores: one per delay, reject and error bit, in that
    /// order and in transition order.
    pub fn value_count(&self) -> usize {
        (self.delay.count_ones() + self.reject.count_ones() + self.error.count_ones()) as usize
    }
}

/// What an applied op changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdfAppliedTarget {
    /// Input terminal `input` to output terminal `output` of declared path
    /// `path` of `scope`: twelve transitions with pulse limits.
    Path {
        scope: u32,
        path: u32,
        input: u32,
        output: u32,
    },
    /// The interconnect delay into load port `load` from `source` (`None`:
    /// from every source; PORT, NETDELAY, or an INTERCONNECT whose source
    /// was not found).
    Port {
        load: SdfPortTarget,
        source: Option<SdfPortTarget>,
    },
    /// A gate or UDP instance: rise, fall and turn-off (bits 0, 1, 2).
    Primitive { scope: u32 },
    /// Limit `slot` (`First` or `Second`) of declared check `check`: one
    /// signed value (bit 0).
    Check {
        scope: u32,
        check: u32,
        slot: SdfLimitSlot,
    },
}

/// One valued op.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdfAppliedOp {
    pub pos: SdfPos,
    pub target: SdfAppliedTarget,
    /// INCREMENT: the values are added; otherwise they replace.
    pub increment: bool,
    /// PATHPULSEPERCENT: the limits are percentages of the delay in
    /// millionths of a percent.
    pub percent: bool,
    pub masks: SdfMasks,
    /// The first of the op's values in [`SdfAnnotation::values`].
    pub first: u32,
}

/// A plan valued for one call: ops in file order, their values, and the
/// warnings valuing gave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdfAnnotation {
    pub ops: Vec<SdfAppliedOp>,
    pub values: Vec<i64>,
    pub warnings: Vec<SdfWarning>,
}

impl SdfAnnotation {
    /// The values of `op`, in [`SdfMasks::value_count`] order.
    pub fn values_of(&self, op: &SdfAppliedOp) -> &[i64] {
        let first = op.first as usize;
        &self.values[first..first + op.masks.value_count()]
    }
}

/// Twelve-transition order of Table 32-4 (the same as 30.5.1): 0->1, 1->0,
/// 0->z, z->1, 1->z, z->0, 0->x, x->1, 1->x, x->0, x->z, z->x.
const TRANSITIONS: usize = 12;

/// How a transition's value derives from the SDF values `v1..vn` (0-based).
#[derive(Clone, Copy)]
enum Source {
    Value(usize),
    Min(usize, usize),
    Max(usize, usize),
}

/// Table 32-4 for 1, 2, 3, 6 and 12 values.
fn transition_sources(count: usize) -> [Source; TRANSITIONS] {
    use Source::{Max, Min, Value as V};
    match count {
        1 => [V(0); TRANSITIONS],
        2 => [
            V(0),
            V(1),
            V(0),
            V(0),
            V(1),
            V(1),
            V(0),
            V(0),
            V(1),
            V(1),
            Max(0, 1),
            Min(0, 1),
        ],
        3 => [
            V(0),
            V(1),
            V(2),
            V(0),
            V(2),
            V(1),
            Min(0, 2),
            V(0),
            Min(1, 2),
            V(1),
            V(2),
            Min(0, 1),
        ],
        6 => [
            V(0),
            V(1),
            V(2),
            V(3),
            V(4),
            V(5),
            Min(0, 2),
            Max(0, 3),
            Min(1, 4),
            Max(1, 5),
            Max(2, 4),
            Min(3, 5),
        ],
        _ => std::array::from_fn(V),
    }
}

/// One delval valued: its delay and its pulse limits.
#[derive(Clone, Copy, Debug, Default)]
struct Valued {
    delay: Option<i64>,
    limits: Limits,
}

#[derive(Clone, Copy, Debug, Default)]
enum Limits {
    /// Derived from the pulse percentages of the new delay.
    #[default]
    Derive,
    /// Given; `None` keeps the current limit.
    Given {
        reject: Option<i64>,
        error: Option<i64>,
    },
}

struct Valuer<'a> {
    options: &'a SdfCallOptions,
    timescale_fs: u64,
    design_precision_fs: u64,
    warnings: &'a mut Vec<SdfWarning>,
}

/// `10^exponent` for exponents a `u128` holds.
fn power_of_ten(exponent: u32) -> Option<i128> {
    10i128.checked_pow(exponent)
}

/// The decimal exponent of `value` when it is a power of ten.
fn decimal_exponent(value: u64) -> Option<u32> {
    let mut exponent = 0;
    let mut rest = value;
    while rest >= 10 && rest.is_multiple_of(10) {
        rest /= 10;
        exponent += 1;
    }
    (rest == 1).then_some(exponent)
}

impl Valuer<'_> {
    /// Ticks of `number * scale` SDF time units, rounded to `precision_fs`
    /// half away from zero. Out of the tick range, one precision unit, with
    /// a warning (a positive delay is never lost).
    fn ticks(
        &mut self,
        pos: SdfPos,
        number: SdfNumber,
        scale: SdfNumber,
        precision_fs: u64,
    ) -> i64 {
        let precision_fs = precision_fs.max(self.design_precision_fs);
        let unit_ticks = (precision_fs / self.design_precision_fs.max(1)) as i64;
        match self.exact_ticks(number, scale, precision_fs) {
            Some(ticks) => ticks,
            None => {
                self.warnings.push(SdfWarning::new(
                    pos,
                    format!(
                        "the value {number} is out of the simulation time range; it is \
                         annotated as one precision unit"
                    ),
                ));
                unit_ticks
            }
        }
    }

    fn exact_ticks(&self, number: SdfNumber, scale: SdfNumber, precision_fs: u64) -> Option<i64> {
        // value = mantissa * 10^exponent femtoseconds.
        let multiplier = self.timescale_fs;
        let timescale_exponent = decimal_exponent(multiplier)?;
        let precision_exponent = decimal_exponent(precision_fs)?;
        let mantissa = i128::from(number.mantissa()) * i128::from(scale.mantissa());
        let exponent = number.exponent() + scale.exponent() + timescale_exponent as i32
            - precision_exponent as i32;
        // Units of the module precision, rounded half away from zero.
        let units = if mantissa == 0 {
            0
        } else if exponent >= 0 {
            mantissa.checked_mul(power_of_ten(exponent as u32)?)?
        } else {
            let shift = (-exponent) as u32;
            match power_of_ten(shift) {
                None => 0,
                Some(divisor) => {
                    let quotient = mantissa / divisor;
                    let remainder = (mantissa % divisor).abs();
                    if remainder * 2 >= divisor {
                        quotient + mantissa.signum()
                    } else {
                        quotient
                    }
                }
            }
        };
        let unit_ticks = i128::from(precision_fs / self.design_precision_fs.max(1));
        i64::try_from(units.checked_mul(unit_ticks)?).ok()
    }

    fn value(&mut self, pos: SdfPos, value: Option<&SdfValue>, precision_fs: u64) -> Option<i64> {
        let (number, scale) = self.options.select(value?)?;
        Some(self.ticks(pos, number, scale, precision_fs))
    }

    /// A PATHPULSEPERCENT value in millionths of a percent.
    fn percent(&mut self, value: Option<&SdfValue>) -> Option<i64> {
        let (number, _) = self.options.select(value?)?;
        let mantissa = i128::from(number.mantissa());
        let exponent = number.exponent() + 6;
        let units = if exponent >= 0 {
            mantissa.checked_mul(power_of_ten(exponent as u32)?)?
        } else {
            mantissa / power_of_ten((-exponent) as u32).unwrap_or(i128::MAX)
        };
        i64::try_from(units.clamp(0, 100_000_000)).ok()
    }

    fn delvals(&mut self, pos: SdfPos, list: &SdfDelvalList, precision_fs: u64) -> Vec<Valued> {
        list.values
            .iter()
            .map(|delval| {
                let delay = self.value(pos, delval.delay.as_ref(), precision_fs);
                let limits = match &delval.pulse {
                    SdfPulse::Unspecified => Limits::Derive,
                    SdfPulse::Limits(limits) => Limits::Given {
                        reject: self.value(pos, limits.reject.as_ref(), precision_fs),
                        error: self.value(pos, limits.error.as_ref(), precision_fs),
                    },
                };
                Valued { delay, limits }
            })
            .collect()
    }
}

/// Accumulates ops and their values.
struct Builder {
    annotation: SdfAnnotation,
}

impl Builder {
    fn push(
        &mut self,
        pos: SdfPos,
        target: SdfAppliedTarget,
        increment: bool,
        percent: bool,
        transitions: &[(Option<i64>, Limits)],
    ) {
        let mut masks = SdfMasks::default();
        let first = self.annotation.values.len() as u32;
        for (index, (delay, _)) in transitions.iter().enumerate() {
            if let Some(delay) = delay {
                masks.delay |= 1 << index;
                self.annotation.values.push(*delay);
            }
        }
        let mut rejects = Vec::new();
        let mut errors = Vec::new();
        for (index, (delay, limits)) in transitions.iter().enumerate() {
            match limits {
                Limits::Derive => {
                    if delay.is_some() {
                        masks.derive |= 1 << index;
                    }
                }
                Limits::Given { reject, error } => {
                    if let Some(reject) = reject {
                        masks.reject |= 1 << index;
                        rejects.push(*reject);
                    }
                    if let Some(error) = error {
                        masks.error |= 1 << index;
                        errors.push(*error);
                    }
                }
            }
        }
        if masks.is_empty() {
            self.annotation.values.truncate(first as usize);
            return;
        }
        self.annotation.values.extend(rejects);
        self.annotation.values.extend(errors);
        self.annotation.ops.push(SdfAppliedOp {
            pos,
            target,
            increment,
            percent,
            masks,
            first,
        });
    }
}

/// The twelve transitions of a delay list (Table 32-4). A transition whose
/// sources include a missing value keeps its value; the limits of a min/max
/// transition are those of the value it took.
fn expand_transitions(values: &[Valued]) -> [(Option<i64>, Limits); TRANSITIONS] {
    // Lists of 4 or 5 values are six with trailing nulls; 7 to 11 twelve.
    let count = match values.len() {
        0 => return [(None, Limits::Derive); TRANSITIONS],
        4 | 5 => 6,
        7..=11 => TRANSITIONS,
        count => count,
    };
    let at = |index: usize| values.get(index).copied().unwrap_or_default();
    transition_sources(count).map(|source| match source {
        Source::Value(index) => {
            let value = at(index);
            (value.delay, value.limits)
        }
        Source::Min(a, b) | Source::Max(a, b) => {
            let (first, second) = (at(a), at(b));
            match (first.delay, second.delay) {
                (Some(x), Some(y)) => {
                    let take_first = match source {
                        Source::Min(..) => x <= y,
                        _ => x >= y,
                    };
                    let chosen = if take_first { first } else { second };
                    (chosen.delay, chosen.limits)
                }
                _ => (None, Limits::Derive),
            }
        }
    })
}

/// The three gate delays of a delay list (Table 28-9): one value for all,
/// two for rise and fall with the turn-off the smaller (or as `turnoff`
/// says), three as given, more than three reduced to the first three.
fn gate_delays(values: &[Valued], turnoff: SdfTurnoff) -> [Option<i64>; 3] {
    let at = |index: usize| values.get(index).and_then(|value| value.delay);
    match values.len() {
        0 => [None; 3],
        1 => [at(0); 3],
        2 => {
            let (rise, fall) = (at(0), at(1));
            let off = rise.zip(fall).map(|(rise, fall)| match turnoff {
                SdfTurnoff::FromFile | SdfTurnoff::Minimum => rise.min(fall),
                SdfTurnoff::Maximum => rise.max(fall),
                SdfTurnoff::Average => (rise + fall) / 2,
            });
            [rise, fall, off]
        }
        _ => [at(0), at(1), at(2)],
    }
}

/// Value `plan` for one call. `timebase` gives the design precision;
/// `targets` the precision of each scope ([`super::SdfScope::precision_fs`]).
pub fn apply(
    plan: &SdfPlan,
    targets: &SdfTargets,
    options: &SdfCallOptions,
    timebase: SdfTimebase,
) -> SdfAnnotation {
    let mut warnings = Vec::new();
    let mut builder = Builder {
        annotation: SdfAnnotation::default(),
    };
    let mut valuer = Valuer {
        options,
        timescale_fs: plan.timescale.femtoseconds(),
        design_precision_fs: timebase.design_precision_fs,
        warnings: &mut warnings,
    };
    let precision = |scope: u32| targets.scope(scope).precision_fs;
    for op in &plan.ops {
        let pos = op.pos;
        match &op.kind {
            SdfOpKind::PathDelay {
                scope,
                path,
                input,
                output,
                delay,
            } => {
                let transitions =
                    path_transitions(&mut valuer, plan, pos, *delay, precision(*scope));
                builder.push(
                    pos,
                    SdfAppliedTarget::Path {
                        scope: *scope,
                        path: *path,
                        input: *input,
                        output: *output,
                    },
                    delay.mode == SdfMode::Increment,
                    false,
                    &transitions,
                );
            }
            SdfOpKind::PathPulse {
                scope,
                path,
                input,
                output,
                pulse,
            } => {
                // One value sets both limits (OVI SDF 3.0 3-15).
                let error = pulse.error.as_ref().or(pulse.reject.as_ref());
                let (reject, error) = if pulse.percent {
                    (valuer.percent(pulse.reject.as_ref()), valuer.percent(error))
                } else {
                    let precision_fs = precision(*scope);
                    (
                        valuer.value(pos, pulse.reject.as_ref(), precision_fs),
                        valuer.value(pos, error, precision_fs),
                    )
                };
                let transitions = [(None, Limits::Given { reject, error }); TRANSITIONS];
                builder.push(
                    pos,
                    SdfAppliedTarget::Path {
                        scope: *scope,
                        path: *path,
                        input: *input,
                        output: *output,
                    },
                    false,
                    pulse.percent,
                    &transitions,
                );
            }
            SdfOpKind::PortDelay { load, delay } | SdfOpKind::NetDelay { load, delay } => {
                let transitions =
                    path_transitions(&mut valuer, plan, pos, *delay, precision(load.scope));
                builder.push(
                    pos,
                    SdfAppliedTarget::Port {
                        load: *load,
                        source: None,
                    },
                    delay.mode == SdfMode::Increment,
                    false,
                    &transitions,
                );
            }
            SdfOpKind::Interconnect {
                source,
                load,
                delay,
            } => {
                let transitions =
                    path_transitions(&mut valuer, plan, pos, *delay, precision(load.scope));
                builder.push(
                    pos,
                    SdfAppliedTarget::Port {
                        load: *load,
                        source: *source,
                    },
                    delay.mode == SdfMode::Increment,
                    false,
                    &transitions,
                );
            }
            SdfOpKind::PrimitiveDelay { scope, delay } => {
                let values = valuer.delvals(pos, plan.delay_list(*delay), precision(*scope));
                let delays = gate_delays(&values, options.turnoff);
                let transitions = delays.map(|delay| {
                    (
                        delay,
                        Limits::Given {
                            reject: None,
                            error: None,
                        },
                    )
                });
                builder.push(
                    pos,
                    SdfAppliedTarget::Primitive { scope: *scope },
                    delay.mode == SdfMode::Increment,
                    false,
                    &transitions,
                );
            }
            SdfOpKind::CheckLimit {
                scope,
                check,
                slot,
                value,
            } => {
                let value = valuer.value(pos, Some(value), precision(*scope));
                let transitions = [(
                    value,
                    Limits::Given {
                        reject: None,
                        error: None,
                    },
                )];
                builder.push(
                    pos,
                    SdfAppliedTarget::Check {
                        scope: *scope,
                        check: *check,
                        slot: *slot,
                    },
                    false,
                    false,
                    &transitions,
                );
            }
        }
    }
    builder.annotation.warnings = warnings;
    builder.annotation
}

fn path_transitions(
    valuer: &mut Valuer<'_>,
    plan: &SdfPlan,
    pos: SdfPos,
    delay: SdfDelayOp,
    precision_fs: u64,
) -> [(Option<i64>, Limits); TRANSITIONS] {
    let values = valuer.delvals(pos, plan.delay_list(delay), precision_fs);
    expand_transitions(&values)
}

#[cfg(test)]
mod tests;
