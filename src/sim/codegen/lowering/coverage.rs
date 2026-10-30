//! Functional coverage (IEEE 1800-2009 19, SIM-044).
//!
//! A constructed covergroup declaration lowers to:
//! - a synthetic class layout whose objects are the covergroup instances
//!   (handles, null checks and collection reuse the class-object ABI): the
//!   runtime record, the owning object of an embedded group, the binding
//!   variant of `ref` formals, then one field per input formal;
//! - an [`IrCovergroup`] table entry with the coverpoint bin layouts;
//! - a `sample` method whose body evaluates every coverpoint once;
//! - for a clocking event, one sampler process that samples every
//!   registered instance at each event, and for a block event, samplers
//!   injected into the named subroutine.
//!
//! Declarations never constructed produce nothing, so a model without
//! covergroup instances pays nothing.
use super::*;
use crate::core::db::{
    CoverageBinKind, CoverageEvent, CoverageRepeat, CoverageSelect, CoverageSelectKind,
    CovergroupInfo, CoverpointInfo, EventSpec, TypeId,
};
use crate::sim::ir::{
    IrCapture, IrClass, IrClassField, IrClassFieldType, IrCoverAutomatic, IrCoverBin,
    IrCoverBinKind, IrCoverBinShape, IrCoverBound, IrCoverCount, IrCoverCross, IrCoverCrossBin,
    IrCoverItem, IrCoverRepeat, IrCoverSelect, IrCoverageCrossSample, IrCoverageOption,
    IrCoverageQuery, IrCoverageSample, IrCoverageStmt, IrCoverageText, IrCoverageTextQuery,
    IrCoverageType, IrCoverageTypeItem, IrCoverageTypeOption, IrCovergroup, IrCoverpoint, IrFormal,
    IrFunc, IrNativeAccessKind, IrObjectQuery, IrPlusArgTarget, IrPreFn, IrProcess, IrShape,
    IrStringExpr, IrUniquePriorityCheck,
};

/// Lowering state of one constructed covergroup declaration.
#[derive(Clone, Debug)]
pub(super) struct CovergroupLowering {
    /// Synthetic class layout of the instances.
    class: usize,
    /// [`IrModel::covergroups`] index.
    ir: usize,
    /// Synthetic `sample` method.
    sample: usize,
    record_field: usize,
    owner_field: Option<usize>,
    /// Field holding the binding variant of `ref` formals.
    bind_field: Option<usize>,
    /// `ref` formal bindings, one map per variant, and each `new` site's
    /// variant.
    variants: Vec<HashMap<NodeId, NodeId>>,
    variant_of_new: HashMap<NodeId, u32>,
    /// Coverpoint declaration -> coverpoint index.
    points: HashMap<NodeId, u32>,
    /// Bin guards and argument slots of the bins.
    build: CoverageBuild,
    /// Instances are sampled through the group registry: a block event, or
    /// a clocking event on static storage.
    registered: bool,
    /// The clocking event reads storage of one instance (a covergroup
    /// argument or a property of the owner): every instance gets its own
    /// waiter process (see [`Codegen::emit_covergroup_waiter`]).
    instance_event: bool,
    /// `type_option.strobe` with a clocking event (19.7.1).
    strobe: bool,
    /// Captured helper of the sample method that waits for one instance's
    /// clocking event, started by every `new`.
    waiter: Option<String>,
}

/// The bin guards and argument slots of one covergroup's bins, collected
/// while its coverpoint tables are built.
#[derive(Clone, Debug, Default)]
pub(super) struct CoverageBuild {
    /// The scope path run-time messages name bins by.
    path: String,
    /// Bin `iff` expressions, evaluated by every sample.
    guards: Vec<NodeId>,
    /// Bins values read from covergroup arguments, evaluated by `new`.
    slots: Vec<NodeId>,
    /// The group sets option.detect_overlap to a value that may be true.
    detect_overlap: bool,
}

impl CoverageBuild {
    fn slot(&mut self, node: NodeId) -> u32 {
        let index = self
            .slots
            .iter()
            .position(|slot| *slot == node)
            .unwrap_or_else(|| {
                self.slots.push(node);
                self.slots.len() - 1
            });
        index as u32
    }
}

/// One constant bins bound, or why it is not one.
#[derive(Clone, Copy, Debug)]
enum CoverageBound {
    Number(i128),
    /// `$`.
    Dollar,
    /// A constant with X or Z bits.
    Unknown,
    /// A value only an instance knows (a covergroup argument).
    Instance,
}

impl CoverageBound {
    fn number(self, dollar: i128) -> Option<i128> {
        match self {
            Self::Number(number) => Some(number),
            Self::Dollar => Some(dollar),
            Self::Unknown | Self::Instance => None,
        }
    }
}

/// The most boxes (conjunctions of select conditions) one cross bin's
/// select expression may expand to.
const MAX_SELECT_BOXES: u64 = 1 << 10;

/// The most key ranges one wildcard bins value may expand to (a power of
/// two); a pattern with more open bits above its lowest fixed bit is
/// rejected.
const WILDCARD_RANGE_LIMIT: u64 = 1 << 12;

/// Identity of a covergroup type: the elaborated declaration, so the
/// covergroup declared in each module instance is a type of its own
/// (decision S44-D1).
pub(super) type CoverageTypeKey = NodeId;

const COVERAGE_RECORD_FIELD: &str = "llg_cov_record";

mod control;
mod fsm;
mod transitions;
pub(super) use control::{CoverageInstances, ImmediateAssertionItem};
const COVERAGE_OWNER_FIELD: &str = "llg_cov_owner";
const COVERAGE_BIND_FIELD: &str = "llg_cov_bind";

/// The covergroup, coverpoint or cross a built-in `option` or
/// `type_option` member belongs to.
#[derive(Clone, Copy, Debug)]
pub(super) struct CoverageOptionOwner {
    group: NodeId,
    point: Option<NodeId>,
    type_option: bool,
}

/// One procedural option access (19.7, 19.7.1).
#[derive(Clone, Debug)]
struct CoverageOptionAccess {
    owner: CoverageOptionOwner,
    member: String,
    /// The instance handle of an `option` access.
    handle: Option<NodeId>,
}

/// The type options a declaration sets (Table 19-3).
struct CoverageTypeDeclared {
    weight: i64,
    goal: i64,
    comment: String,
    strobe: bool,
    merge: bool,
}

/// The built-in coverage method `callee` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoverageMethod {
    Sample,
    GetCoverage,
    GetInstCoverage,
    Start,
    Stop,
    SetInstName,
}

/// The integral instance option a member of `option` names (19.10).
fn coverage_instance_option(member: &str) -> Option<IrCoverageOption> {
    Some(match member {
        "weight" => IrCoverageOption::Weight,
        "goal" => IrCoverageOption::Goal,
        "at_least" => IrCoverageOption::AtLeast,
        "auto_bin_max" => IrCoverageOption::AutoBinMax,
        "detect_overlap" => IrCoverageOption::DetectOverlap,
        "get_inst_coverage" => IrCoverageOption::GetInstCoverage,
        "per_instance" => IrCoverageOption::PerInstance,
        "cross_num_print_missing" => IrCoverageOption::CrossNumPrintMissing,
        _ => return None,
    })
}

/// The integral type option a member of `type_option` names (19.10).
fn coverage_type_option(member: &str) -> Option<IrCoverageTypeOption> {
    Some(match member {
        "weight" => IrCoverageTypeOption::Weight,
        "goal" => IrCoverageTypeOption::Goal,
        "strobe" => IrCoverageTypeOption::Strobe,
        "merge_instances" => IrCoverageTypeOption::MergeInstances,
        _ => return None,
    })
}

impl<'a> Codegen<'a> {
    fn coverage_error(&self, node: NodeId, message: impl std::fmt::Display) -> String {
        format!("{}: {message}", self.source_location(node))
    }

    /// The name a diagnostic uses for a covergroup: its declared name, or
    /// the instance variable of an embedded group.
    fn covergroup_name(&self, group: NodeId) -> String {
        let name = &self.node(group).name;
        if !name.is_empty() {
            return name.clone();
        }
        let info = self.db.covergroup(group);
        self.node(group)
            .parent
            .and_then(|class| {
                self.node(class).children.iter().copied().find(|child| {
                    matches!(self.kind(*child), NodeKind::Var { .. })
                        && info.and_then(|info| info.type_id).is_some_and(|type_id| {
                            self.db
                                .type_descriptor(*child)
                                .is_some_and(|descriptor| descriptor.id == type_id)
                        })
                })
            })
            .map(|property| self.node(property).name.clone())
            .unwrap_or_else(|| "<embedded covergroup>".to_owned())
    }

    /// The covergroup a handle-typed declaration or expression refers to.
    pub(in super::super) fn covergroup_of_type(&self, type_id: Option<TypeId>) -> Option<NodeId> {
        type_id.and_then(|type_id| self.db.covergroup_for_type(type_id))
    }

    fn coverage_type_index(&mut self, group: NodeId) -> usize {
        let key: CoverageTypeKey = group;
        if let Some(index) = self.coverage_types.get(&key) {
            return *index;
        }
        let index = self.model.coverage_types.len();
        let (options, coverpoints, crosses) = self
            .db
            .covergroup(group)
            .map(|info| {
                (
                    info.options.clone(),
                    info.coverpoints.clone(),
                    info.crosses.clone(),
                )
            })
            .unwrap_or_default();
        let declared = self.coverage_type_declared(&options);
        let items = coverpoints
            .iter()
            .map(|point| {
                self.db
                    .coverpoint(*point)
                    .map(|metadata| metadata.options.clone())
                    .unwrap_or_default()
            })
            .chain(crosses.iter().map(|cross| {
                self.db
                    .coverage_cross(*cross)
                    .map(|metadata| metadata.options.clone())
                    .unwrap_or_default()
            }))
            .map(|setters| {
                let item = self.coverage_type_declared(&setters);
                IrCoverageTypeItem {
                    weight: item.weight,
                    goal: item.goal,
                    comment: item.comment,
                }
            })
            .collect();
        let name = self.covergroup_name(group);
        self.model.coverage_types.push(IrCoverageType {
            name,
            weight: declared.weight,
            merge: declared.merge,
            goal: declared.goal,
            comment: declared.comment,
            strobe: declared.strobe,
            items,
        });
        self.coverage_types.insert(key, index);
        index
    }

    /// The type options one covergroup, coverpoint or cross declares
    /// (Table 19-3); the frontend requires constant values (19.7.1).
    fn coverage_type_declared(&self, setters: &[NodeId]) -> CoverageTypeDeclared {
        let mut declared = CoverageTypeDeclared {
            weight: 1,
            goal: 100,
            comment: String::new(),
            strobe: false,
            merge: false,
        };
        for setter in setters {
            let Some((scope, member, value)) = self.coverage_option_parts(*setter) else {
                continue;
            };
            if scope != "type_option" {
                continue;
            }
            let number = || {
                self.eval_bound_i128(value)
                    .ok()
                    .and_then(|value| i64::try_from(value).ok())
            };
            match member.as_str() {
                "weight" => declared.weight = number().unwrap_or(1),
                "goal" => declared.goal = number().unwrap_or(100),
                "strobe" => declared.strobe = number().is_some_and(|value| value != 0),
                "merge_instances" => declared.merge = number().is_some_and(|value| value != 0),
                "comment" => {
                    declared.comment = self.coverage_constant_text(value).unwrap_or_default()
                }
                _ => {}
            }
        }
        declared
    }

    /// The value of a constant string expression: a literal or a string
    /// parameter.
    fn coverage_constant_text(&self, node: NodeId) -> Option<String> {
        let node = self.p30_unwrap_cast(node);
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { value, .. }) => decoded_string_bytes(value)
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => match self.param_vals.get(target)? {
                Val::Str(text) => Some(text.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// `(option or type_option, member, value)` of one option setter.
    fn coverage_option_parts(&self, setter: NodeId) -> Option<(String, String, NodeId)> {
        let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Assignment,
            operands,
            ..
        }) = self.kind(setter)
        else {
            return None;
        };
        let [target, value] = operands.as_slice() else {
            return None;
        };
        let NodeKind::Expr(ExprKind::HierPath { parts, .. }) = self.kind(*target) else {
            return None;
        };
        let [scope, member] = parts.as_slice() else {
            return None;
        };
        Some((scope.clone(), member.clone(), *value))
    }

    /// Record the built-in `option` and `type_option` members of every
    /// covergroup, coverpoint and cross, which procedural option accesses
    /// name (19.7, 19.7.1).
    fn collect_option_vars(&mut self) {
        let mut owners = HashMap::new();
        for (group, info) in self.db.covergroups() {
            let scopes = info
                .body
                .map(|body| (body, None))
                .into_iter()
                .chain(info.coverpoints.iter().map(|point| (*point, Some(*point))))
                .chain(info.crosses.iter().map(|cross| (*cross, Some(*cross))));
            for (scope, point) in scopes {
                for child in &self.node(scope).children {
                    if !matches!(self.kind(*child), NodeKind::Var { .. }) {
                        continue;
                    }
                    let type_option = match self.node(*child).name.as_str() {
                        "option" => false,
                        "type_option" => true,
                        _ => continue,
                    };
                    owners.insert(
                        *child,
                        CoverageOptionOwner {
                            group: *group,
                            point,
                            type_option,
                        },
                    );
                }
            }
        }
        self.coverage_option_vars = owners;
    }

    /// The option a hierarchical name selects: `h.option.m`,
    /// `h.cp.option.m`, `cg::type_option.m` or `cg::cp::type_option.m`.
    fn coverage_option_access(&self, node: NodeId) -> Option<CoverageOptionAccess> {
        if self.coverage_option_vars.is_empty() {
            return None;
        }
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return None;
        };
        let at = refs.iter().position(|target| {
            target.is_some_and(|target| self.coverage_option_vars.contains_key(&target))
        })?;
        let owner = self.coverage_option_vars[&refs[at]?];
        // Only a member is an option; the whole structure is not.
        if at + 2 != parts.len() {
            return None;
        }
        let member = parts[at + 1].clone();
        // An instance option names the instance before the coverpoint.
        let prefix = at.checked_sub(usize::from(owner.point.is_some()));
        let handle = match prefix {
            Some(prefix) if prefix > 0 && !owner.type_option => {
                let mut current = node;
                loop {
                    current = *self.node(current).children.first()?;
                    match self.kind(current) {
                        NodeKind::Expr(ExprKind::HierPath { parts, .. })
                            if parts.len() > prefix =>
                        {
                            continue
                        }
                        NodeKind::Expr(ExprKind::HierPath { parts, .. })
                            if parts.len() == prefix =>
                        {
                            break Some(current)
                        }
                        NodeKind::Expr(ExprKind::HierPath { .. }) => return None,
                        _ if prefix == 1 => break Some(current),
                        _ => return None,
                    }
                }
            }
            _ => None,
        };
        Some(CoverageOptionAccess {
            owner,
            member,
            handle,
        })
    }

    /// Whether `node` reads a string coverage option (`option.name`,
    /// `option.comment`, `type_option.comment`).
    pub(in super::super) fn is_coverage_text(&self, node: NodeId) -> bool {
        self.coverage_option_access(node)
            .is_some_and(|access| matches!(access.member.as_str(), "name" | "comment"))
    }

    /// The index of a coverpoint or cross among the group's coverage items
    /// (coverpoints, then crosses).
    fn coverage_item_position(
        &self,
        node: NodeId,
        group: NodeId,
        point: Option<NodeId>,
    ) -> Result<Option<u32>, String> {
        let Some(point) = point else {
            return Ok(None);
        };
        self.db
            .covergroup(group)
            .and_then(|info| {
                info.coverpoints
                    .iter()
                    .chain(&info.crosses)
                    .position(|id| *id == point)
            })
            .map(|index| Some(index as u32))
            .ok_or_else(|| self.coverage_error(node, "coverpoint is not in its covergroup"))
    }

    /// The runtime record of the instance an `option` access names.
    fn coverage_option_record(
        &mut self,
        path: &str,
        node: NodeId,
        access: &CoverageOptionAccess,
    ) -> Result<IrChandleExpr, String> {
        let handle_node = access.handle.ok_or_else(|| {
            self.coverage_error(
                node,
                "covergroup instance options can only be accessed through a covergroup instance outside the covergroup (llg limitation)",
            )
        })?;
        let handle = self.lower_chandle(path, handle_node)?;
        let object = IrChandleExpr::Required {
            handle: Box::new(handle),
            site: self.source_site("covergroup option access", path, node),
        };
        self.coverage_record(access.owner.group, object)
    }

    /// An integral option read (19.7, 19.10).
    fn lower_coverage_option_read(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(access) = self.coverage_option_access(node) else {
            return Ok(None);
        };
        let point = self.coverage_item_position(node, access.owner.group, access.owner.point)?;
        let query = if access.owner.type_option {
            let option = coverage_type_option(&access.member).ok_or_else(|| {
                self.coverage_error(
                    node,
                    format!(
                        "type_option.{} is not an integral coverage option",
                        access.member
                    ),
                )
            })?;
            IrCoverageQuery::TypeOption {
                coverage_type: self.coverage_type_index(access.owner.group),
                point,
                option,
            }
        } else {
            let option = coverage_instance_option(&access.member).ok_or_else(|| {
                self.coverage_error(
                    node,
                    format!(
                        "option.{} is not an integral coverage option",
                        access.member
                    ),
                )
            })?;
            let record = self.coverage_option_record(path, node, &access)?;
            IrCoverageQuery::Option {
                record,
                point,
                option,
            }
        };
        let (width, signed) = query.result_type();
        Ok(Some(IrExpr::new(
            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::Coverage(Box::new(query)))),
            width,
            signed,
            None,
        )))
    }

    /// A string option read: `option.name`, `option.comment` or
    /// `type_option.comment` (19.7, 19.10).
    pub(in super::super) fn lower_coverage_text(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        let Some(access) = self.coverage_option_access(node) else {
            return Ok(None);
        };
        let point = self.coverage_item_position(node, access.owner.group, access.owner.point)?;
        let query = match (access.owner.type_option, access.member.as_str()) {
            (true, "comment") => IrCoverageTextQuery::Type {
                coverage_type: self.coverage_type_index(access.owner.group),
                point,
            },
            (false, "name" | "comment") => {
                let which = if access.member == "name" {
                    IrCoverageText::Name
                } else {
                    IrCoverageText::Comment
                };
                let record = self.coverage_option_record(path, node, &access)?;
                IrCoverageTextQuery::Instance {
                    record,
                    point,
                    which,
                }
            }
            _ => {
                return Err(self.coverage_error(
                    node,
                    format!("coverage option `{}` is not a string", access.member),
                ))
            }
        };
        Ok(Some(IrStringExpr::CoverageText(Box::new(query))))
    }

    /// A procedural option write (19.7, 19.7.1), or `None` when `lhs` names
    /// no option.
    pub(in super::super) fn lower_coverage_option_write(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let Some(access) = self.coverage_option_access(lhs) else {
            return Ok(None);
        };
        let scope = if access.owner.type_option {
            "type_option"
        } else {
            "option"
        };
        let member = access.member.as_str();
        if !blocking || op != Operation::Assignment {
            return Err(self.coverage_error(
                lhs,
                format!(
                    "{scope}.{member} can only be written by a plain blocking assignment (llg limitation)"
                ),
            ));
        }
        let definition_only = if access.owner.type_option {
            (member == "strobe").then_some("19.7.1")
        } else {
            matches!(
                member,
                "per_instance" | "get_inst_coverage" | "auto_bin_max" | "detect_overlap"
            )
            .then_some("19.7")
        };
        if let Some(clause) = definition_only {
            return Err(self.coverage_error(
                lhs,
                format!(
                    "{scope}.{member} can only be set in the covergroup definition (IEEE 1800-2009 {clause})"
                ),
            ));
        }
        let point = self.coverage_item_position(lhs, access.owner.group, access.owner.point)?;
        let site = self.source_site(&format!("{scope}.{member}"), path, lhs);
        let text = matches!(member, "name" | "comment");
        let statement = match (access.owner.type_option, text) {
            (true, true) => IrCoverageStmt::TypeText {
                coverage_type: self.coverage_type_index(access.owner.group),
                point,
                value: self.lower_string(path, rhs)?,
            },
            (true, false) => {
                let option = coverage_type_option(member).ok_or_else(|| {
                    self.coverage_error(
                        lhs,
                        format!("type_option.{member} is not a coverage option"),
                    )
                })?;
                let value = self.lower_coverage_option_value(path, rhs, scope, member)?;
                IrCoverageStmt::TypeOption {
                    coverage_type: self.coverage_type_index(access.owner.group),
                    point,
                    option,
                    value,
                    site,
                }
            }
            (false, true) => {
                let record = self.coverage_option_record(path, lhs, &access)?;
                IrCoverageStmt::Text {
                    record,
                    point,
                    which: if member == "name" {
                        IrCoverageText::Name
                    } else {
                        IrCoverageText::Comment
                    },
                    value: self.lower_string(path, rhs)?,
                }
            }
            (false, false) => {
                let option = coverage_instance_option(member).ok_or_else(|| {
                    self.coverage_error(lhs, format!("option.{member} is not a coverage option"))
                })?;
                let record = self.coverage_option_record(path, lhs, &access)?;
                let value = self.lower_coverage_option_value(path, rhs, scope, member)?;
                IrCoverageStmt::Option {
                    record,
                    point,
                    option,
                    value,
                    site,
                }
            }
        };
        Ok(Some(IrStmt::Coverage(Box::new(statement))))
    }

    fn lower_coverage_option_value(
        &mut self,
        path: &str,
        value: NodeId,
        scope: &str,
        member: &str,
    ) -> Result<IrExpr, String> {
        let lowered = self
            .lower_expr(path, value)
            .map_err(|error| self.coverage_error(value, error))?;
        if lowered.is_real() {
            return Err(
                self.coverage_error(value, format!("{scope}.{member} needs an integral value"))
            );
        }
        Ok(lowered)
    }

    /// `$set_coverage_db_name(name)` and `$load_coverage_db(name)` (19.9).
    pub(in super::super) fn lower_coverage_db_task(
        &mut self,
        path: &str,
        name: &str,
        node: NodeId,
        args: &[NodeId],
    ) -> Result<IrStmt, String> {
        let [file] = args else {
            return Err(self.coverage_error(
                node,
                format!("{name} takes one string argument (IEEE 1800-2009 19.9)"),
            ));
        };
        let file_name = self.lower_string(path, *file)?;
        let statement = if name == "$set_coverage_db_name" {
            IrCoverageStmt::SetDbName { name: file_name }
        } else {
            IrCoverageStmt::LoadDb {
                name: file_name,
                site: self.source_site(name, path, node),
            }
        };
        Ok(IrStmt::Coverage(Box::new(statement)))
    }

    /// Lowered covergroups: every declaration a `new` or an instance method
    /// call names.
    fn constructed_covergroups(&self) -> Vec<NodeId> {
        let mut groups = self
            .db
            .node_ids()
            .filter_map(|id| match self.kind(id) {
                NodeKind::Expr(ExprKind::NewCovergroup { group_type, .. }) => {
                    self.covergroup_of_type(*group_type)
                }
                // A method called on a handle that is never constructed
                // still needs the layout: the call fails on the null handle.
                NodeKind::MethodCall { .. } => self
                    .coverage_call(id)
                    .filter(|(_, _, method)| *method != CoverageMethod::GetCoverage)
                    .map(|(group, _, _)| group),
                // So does an instance option access.
                NodeKind::Expr(ExprKind::HierPath { .. }) => self
                    .coverage_option_access(id)
                    .filter(|access| !access.owner.type_option)
                    .map(|access| access.owner.group),
                _ => None,
            })
            .collect::<Vec<_>>();
        groups.sort_by_key(|group| group.index());
        groups.dedup();
        groups
    }

    /// Build the class layout, coverpoint tables and `sample` prototype of
    /// every constructed covergroup. Runs after class collection, before
    /// any body that may construct or call one is lowered.
    pub(in super::super) fn collect_covergroups(&mut self) -> Result<(), String> {
        if self.db.covergroups().is_empty() {
            return Ok(());
        }
        self.collect_option_vars();
        for group in self.constructed_covergroups() {
            self.collect_covergroup(group)?;
        }
        Ok(())
    }

    fn collect_covergroup(&mut self, group: NodeId) -> Result<(), String> {
        let info =
            self.db.covergroup(group).cloned().ok_or_else(|| {
                self.coverage_error(group, "covergroup has no captured structure")
            })?;
        let name = self.covergroup_name(group);
        if info.extends {
            return Err(self.coverage_error(
                group,
                format!("covergroup `{name}` extends another covergroup, which IEEE 1800-2009 does not define"),
            ));
        }
        let embedded_owner = info
            .embedded
            .then(|| self.node(group).parent)
            .flatten()
            .filter(|owner| self.class_nodes.contains_key(owner));
        if info.embedded && embedded_owner.is_none() {
            return Err(self.coverage_error(
                group,
                format!("embedded covergroup `{name}` has no class layout"),
            ));
        }
        // Class layout.
        let class = self.model.classes.len();
        let mut fields = vec![IrClassField {
            c_name: format!("f_{class}_0_{COVERAGE_RECORD_FIELD}"),
            ty: IrClassFieldType::Chandle,
            container: None,
            native_value: None,
            event: false,
        }];
        let owner_field = embedded_owner.map(|_| {
            fields.push(IrClassField {
                c_name: format!("f_{class}_{}_{COVERAGE_OWNER_FIELD}", fields.len()),
                ty: IrClassFieldType::Chandle,
                container: None,
                native_value: None,
                event: false,
            });
            fields.len() - 1
        });
        let (variants, variant_of_new) = self.coverage_ref_variants(group, &info)?;
        let bind_field = (variants.len() > 1).then(|| {
            fields.push(IrClassField {
                c_name: format!("f_{class}_{}_{COVERAGE_BIND_FIELD}", fields.len()),
                ty: IrClassFieldType::Packed {
                    width: 32,
                    signed: false,
                    two_state: true,
                },
                container: None,
                native_value: None,
                event: false,
            });
            fields.len() - 1
        });
        for formal in &info.formals {
            let NodeKind::FuncArg { direction, ty, .. } = self.kind(*formal) else {
                return Err(self.coverage_error(*formal, "covergroup formal is not an argument"));
            };
            match direction {
                crate::core::db::Direction::Input => {}
                crate::core::db::Direction::Ref => continue,
                _ => {
                    return Err(self.coverage_error(
                        *formal,
                        format!(
                            "covergroup `{name}` formal `{}` must be an input or ref argument (IEEE 1800-2009 19.3)",
                            self.node(*formal).name
                        ),
                    ))
                }
            }
            let ty = ty.clone();
            let field_ty = self.coverage_field_type(*formal, &ty)?;
            let index = fields.len();
            fields.push(IrClassField {
                c_name: format!("f_{class}_{index}_{}", ident(&self.node(*formal).name)),
                ty: field_ty,
                container: None,
                native_value: None,
                event: false,
            });
            self.class_fields.insert(*formal, (class, index));
        }
        self.model.classes.push(IrClass {
            c_name: format!("llg_class_{class}"),
            base: None,
            fields,
        });
        self.class_nodes.insert(group, class);
        // Coverpoint tables.
        let mut points = Vec::with_capacity(info.coverpoints.len());
        let mut point_indices = HashMap::new();
        let mut build = CoverageBuild {
            path: self.source_path(&self.covergroup_path(group)),
            ..CoverageBuild::default()
        };
        build.detect_overlap = info
            .options
            .iter()
            .any(|setter| self.coverage_option_may_be_set(*setter, "detect_overlap"));
        for (index, point) in info.coverpoints.iter().copied().enumerate() {
            let metadata = self.db.coverpoint(point).cloned().ok_or_else(|| {
                self.coverage_error(point, "coverpoint has no captured structure")
            })?;
            points.push(self.coverpoint_table(&name, point, &metadata, &mut build)?);
            point_indices.insert(point, index as u32);
        }
        let mut crosses = Vec::with_capacity(info.crosses.len());
        for (index, cross) in info.crosses.iter().copied().enumerate() {
            let table = self.cross_table(&name, cross, &info, &mut points, &mut build)?;
            crosses.push(table);
            point_indices.insert(cross, (info.coverpoints.len() + index) as u32);
        }
        for setter in &info.options {
            self.check_group_option(&name, *setter)?;
        }
        let coverage_type = self.coverage_type_index(group);
        let clocking = matches!(info.event, Some(CoverageEvent::Clocking { .. }));
        let instance_event = match &info.event {
            Some(CoverageEvent::Clocking { specs, .. }) => {
                let mut nodes = Vec::new();
                for spec in specs {
                    spec.referenced_nodes(&mut nodes);
                }
                nodes
                    .iter()
                    .any(|node| self.coverage_event_reads_instance(&info, *node).is_some())
            }
            _ => false,
        };
        // 19.3: the strobe option only schedules samples a clocking event
        // triggers; block events and `sample()` calls are unaffected.
        let strobe = clocking && self.covergroup_strobe(&info);
        let ir = self.model.covergroups.len();
        self.model.covergroups.push(IrCovergroup {
            name: name.clone(),
            path: format!("{}.{name}", build.path),
            coverage_type,
            points,
            crosses,
            slots: build.slots.len() as u32,
            guards: build.guards.len() as u32,
            strobe: None,
        });
        // The `sample` prototype: a method of the instance class whose formals
        // are the `with function sample` arguments (19.8.1).
        let mut formals = Vec::with_capacity(info.sample_formals.len());
        for formal in &info.sample_formals {
            let NodeKind::FuncArg { ty, .. } = self.kind(*formal) else {
                return Err(self.coverage_error(*formal, "sample formal is not an argument"));
            };
            let width = match ty.width {
                Some(width) if !is_real_kind(&ty.kind) && width > 0 && width <= LLG_MAX_WIDTH => {
                    width
                }
                _ => {
                    return Err(self.coverage_error(
                        *formal,
                        format!(
                            "sample argument `{}` of covergroup `{name}` must be integral",
                            self.node(*formal).name
                        ),
                    ))
                }
            };
            let mut ir_formal =
                IrFormal::new(false, width, ty.signed).map_err(|error| error.to_string())?;
            ir_formal.two_state = self.db.is_two_state_type(*formal) || is_two_state_kind(&ty.kind);
            formals.push(ir_formal);
        }
        let sample = self.model.funcs.len();
        let path = self.covergroup_path(group);
        let mut function = IrFunc::new(
            self.c_name("fn_cov_sample", &path, &[&name]),
            None,
            formals,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        function.is_task = false;
        function.receiver_class = Some(class);
        function.diagnostic_name = Some(format!("{name}::sample"));
        function.origin = crate::sim::semantic::Origin::Synthetic {
            reason: format!("sample method of covergroup {}", self.node(group).full_name),
        };
        self.model.funcs.push(function);
        if strobe {
            self.model.covergroups[ir].strobe = Some(sample);
        }
        let registered = info.event.is_some() && !instance_event;
        self.covergroups.insert(
            group,
            CovergroupLowering {
                class,
                ir,
                sample,
                record_field: 0,
                owner_field,
                bind_field,
                variants,
                variant_of_new,
                points: point_indices,
                build,
                registered,
                instance_event,
                strobe,
                waiter: None,
            },
        );
        Ok(())
    }

    /// Whether the group sets `type_option.strobe` to a nonzero constant.
    fn covergroup_strobe(&self, info: &CovergroupInfo) -> bool {
        info.options.iter().any(|setter| {
            self.coverage_option_parts(*setter)
                .is_some_and(|(scope, member, value)| {
                    scope == "type_option"
                        && member == "strobe"
                        && self.eval_bound_i128(value).is_ok_and(|value| value != 0)
                })
        })
    }

    fn coverage_field_type(
        &self,
        formal: NodeId,
        ty: &crate::core::model::TypeInfo,
    ) -> Result<IrClassFieldType, String> {
        if ty.kind == "string" {
            return Ok(IrClassFieldType::String);
        }
        if is_real_kind(&ty.kind) {
            return Ok(IrClassFieldType::Real {
                shortreal: ty.kind == "shortreal",
            });
        }
        if is_handle_kind(&ty.kind) {
            return Ok(IrClassFieldType::Chandle);
        }
        match ty.width {
            Some(width) if width > 0 && width <= LLG_MAX_WIDTH => Ok(IrClassFieldType::Packed {
                width,
                signed: ty.signed,
                two_state: self.db.is_two_state_type(formal) || is_two_state_kind(&ty.kind),
            }),
            _ => Err(self.coverage_error(
                formal,
                format!(
                    "covergroup formal `{}` has no supported packed width",
                    self.node(formal).name
                ),
            )),
        }
    }

    /// The instance path and module instance a covergroup body lowers in.
    fn covergroup_path(&self, group: NodeId) -> String {
        let parent = self.node(group).parent.unwrap_or(group);
        self.instance_path_of(parent)
    }

    fn covergroup_inst(&self, group: NodeId) -> NodeId {
        let parent = self.node(group).parent.unwrap_or(group);
        if self.class_nodes.contains_key(&parent) {
            return parent;
        }
        self.owning_inst(group).unwrap_or(parent)
    }

    /// Bindings of `ref` formals at every `new` of the group. Each distinct
    /// tuple of bound variables is one variant; reads of a `ref` formal
    /// resolve to that variant's variable.
    #[allow(clippy::type_complexity)]
    fn coverage_ref_variants(
        &self,
        group: NodeId,
        info: &CovergroupInfo,
    ) -> Result<(Vec<HashMap<NodeId, NodeId>>, HashMap<NodeId, u32>), String> {
        let refs = info
            .formals
            .iter()
            .enumerate()
            .filter(|(_, formal)| {
                matches!(
                    self.kind(**formal),
                    NodeKind::FuncArg {
                        direction: crate::core::db::Direction::Ref,
                        ..
                    }
                )
            })
            .map(|(index, formal)| (index, *formal))
            .collect::<Vec<_>>();
        let mut variants: Vec<HashMap<NodeId, NodeId>> = Vec::new();
        let mut keys: Vec<Vec<NodeId>> = Vec::new();
        let mut variant_of_new = HashMap::new();
        if refs.is_empty() {
            return Ok((vec![HashMap::new()], variant_of_new));
        }
        for site in self.db.node_ids() {
            let NodeKind::Expr(ExprKind::NewCovergroup { group_type, args }) = self.kind(site)
            else {
                continue;
            };
            if self.covergroup_of_type(*group_type) != Some(group) {
                continue;
            }
            let mut binding = HashMap::new();
            let mut key = Vec::new();
            for (index, formal) in &refs {
                let actual = *args.get(*index).ok_or_else(|| {
                    self.coverage_error(site, "covergroup `new` has fewer arguments than formals")
                })?;
                let target = self.coverage_ref_target(actual).ok_or_else(|| {
                    self.coverage_error(
                        actual,
                        format!(
                            "ref argument `{}` of covergroup `{}` must name a whole static variable or net (llg limitation, see docs/sim_features.md)",
                            self.node(*formal).name,
                            self.covergroup_name(group)
                        ),
                    )
                })?;
                binding.insert(*formal, actual);
                key.push(target);
            }
            let variant = match keys.iter().position(|existing| *existing == key) {
                Some(variant) => variant,
                None => {
                    keys.push(key);
                    variants.push(binding);
                    variants.len() - 1
                }
            };
            variant_of_new.insert(site, variant as u32);
        }
        if variants.is_empty() {
            variants.push(HashMap::new());
        }
        Ok((variants, variant_of_new))
    }

    /// The static variable or net a `ref` actual names, if it is one.
    fn coverage_ref_target(&self, actual: NodeId) -> Option<NodeId> {
        let target = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Ref { target }) => (*target)?,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => (*refs.last()?)?,
            _ => return None,
        };
        if self.class_fields.contains_key(&target) {
            return None;
        }
        match self.kind(target) {
            NodeKind::Var { .. } => (self.db.variable_lifetime(target) == VariableLifetime::Static
                && self.db.array_meta(target).is_none())
            .then_some(target),
            NodeKind::Net { .. } => Some(target),
            _ => None,
        }
    }

    /// Whether `setter` assigns option `member` a value that is not the
    /// constant 0.
    fn coverage_option_may_be_set(&self, setter: NodeId, member: &str) -> bool {
        self.coverage_option_parts(setter)
            .is_some_and(|(scope, name, value)| {
                scope == "option"
                    && name == member
                    && !self.eval_bound_i128(value).is_ok_and(|value| value == 0)
            })
    }

    fn check_group_option(&self, group_name: &str, setter: NodeId) -> Result<(), String> {
        let (scope, member, value) = self.coverage_option_parts(setter).ok_or_else(|| {
            self.coverage_error(setter, "coverage option setter has no option member")
        })?;
        self.check_option(group_name, &scope, &member, value, setter, true)
    }

    /// Reject options whose effect is not implemented; accept those that
    /// only describe reports or the coverage database (19.7).
    fn check_option(
        &self,
        group_name: &str,
        scope: &str,
        member: &str,
        value: NodeId,
        setter: NodeId,
        group_level: bool,
    ) -> Result<(), String> {
        let unsupported = |what: &str| {
            Err(self.coverage_error(
                setter,
                format!("{scope}.{member} in covergroup `{group_name}` {what}"),
            ))
        };
        match (scope, member) {
            // The frontend checks which options each level declares
            // (19.10) and rejects duplicates (19.7).
            ("option", "weight" | "goal" | "at_least" | "auto_bin_max") => Ok(()),
            ("option", "name" | "comment" | "per_instance" | "get_inst_coverage") => Ok(()),
            ("option", "cross_num_print_missing" | "detect_overlap") => Ok(()),
            ("type_option", "weight") => match self.eval_bound_i128(value) {
                Err(_) => unsupported("must be a constant expression (IEEE 1800-2009 19.7.1)"),
                Ok(weight) if weight < 0 => {
                    unsupported("must be a non-negative integral value (IEEE 1800-2009 Table 19-3)")
                }
                Ok(_) => Ok(()),
            },
            ("type_option", "goal" | "comment") => Ok(()),
            ("type_option", "strobe") if !group_level => {
                unsupported("can only be set for a covergroup (IEEE 1800-2009 Table 19-4)")
            }
            ("type_option", "strobe") if self.eval_bound_i128(value).is_err() => {
                unsupported("must be a constant expression (IEEE 1800-2009 19.7.1)")
            }
            ("type_option", "strobe") => Ok(()),
            ("type_option", "merge_instances") if self.eval_bound_i128(value).is_err() => {
                unsupported("must be a constant expression (IEEE 1800-2009 19.7.1)")
            }
            ("type_option", "merge_instances") => Ok(()),
            _ => unsupported("is not a coverage option"),
        }
    }

    /// Bin layout of one coverpoint (19.5, 19.5.2, 19.5.6).
    fn coverpoint_table(
        &mut self,
        group_name: &str,
        point: NodeId,
        info: &CoverpointInfo,
        build: &mut CoverageBuild,
    ) -> Result<IrCoverpoint, String> {
        let point_name = self.coverpoint_name(point);
        let integral = !is_real_kind(&info.ty.kind)
            && !matches!(
                info.ty.kind.as_str(),
                "string" | "class" | "chandle" | "event"
            );
        let width = match info.ty.width {
            Some(width) if integral && (1..=64).contains(&width) => width,
            Some(width) if integral && width > 64 => {
                return Err(self.coverage_error(
                    point,
                    format!(
                        "coverpoint `{point_name}` of covergroup `{group_name}` is {width} bits wide; llg supports coverpoints of at most 64 bits"
                    ),
                ))
            }
            _ => {
                return Err(self.coverage_error(
                    point,
                    format!(
                        "coverpoint `{point_name}` of covergroup `{group_name}` must be an integral expression (IEEE 1800-2009 19.5)"
                    ),
                ))
            }
        };
        let signed = info.ty.signed;
        for setter in &info.options {
            let (scope, member, value) = self.coverage_option_parts(*setter).ok_or_else(|| {
                self.coverage_error(*setter, "coverage option setter has no option member")
            })?;
            self.check_option(group_name, &scope, &member, value, *setter, false)?;
        }
        // 19.5.2: automatic bins only for a coverpoint without `bins`
        // declarations; ignore_bins and illegal_bins alone keep them.
        let declares_bins = info.bins.iter().any(|bin| {
            self.db
                .coverage_bin(*bin)
                .is_some_and(|bin| bin.kind == CoverageBinKind::Bins)
        });
        let automatic = if declares_bins {
            IrCoverAutomatic::None
        } else {
            match self.enum_coverpoint_keys(point, info, width, signed)? {
                Some(keys) => IrCoverAutomatic::Enum(keys),
                None => IrCoverAutomatic::Auto,
            }
        };
        let mut coverpoint = IrCoverpoint {
            name: point_name.clone(),
            width,
            signed,
            automatic,
            bins: Vec::with_capacity(info.bins.len()),
            items: Vec::new(),
            transitions: None,
            crossed: false,
        };
        let mut transition_bins = Vec::new();
        let mut transition_nodes = Vec::new();
        for bin in info.bins.iter().copied() {
            let metadata = self
                .db
                .coverage_bin(bin)
                .ok_or_else(|| self.coverage_error(bin, "bins declaration has no structure"))?;
            if metadata.transition || metadata.default_sequence {
                let spec =
                    self.coverage_transition_bin(group_name, point, bin, width, signed, build)?;
                if spec.sets.is_empty()
                    && transition_bins
                        .iter()
                        .any(|other: &transitions::BinSpec| other.sets.is_empty())
                {
                    return Err(self.coverage_error(
                        bin,
                        format!(
                            "coverpoint `{point_name}` in covergroup `{group_name}` has more than one `default sequence` bin"
                        ),
                    ));
                }
                transition_bins.push(spec);
                transition_nodes.push(bin);
                continue;
            }
            self.coverage_value_bin(group_name, point, bin, &mut coverpoint, build)?;
        }
        if !transition_bins.is_empty() {
            let detect_overlap = build.detect_overlap
                || info
                    .options
                    .iter()
                    .any(|setter| self.coverage_option_may_be_set(*setter, "detect_overlap"));
            let tables = transitions::build(width, transition_bins, detect_overlap).map_err(|(index, message)| {
                let bin = transition_nodes[index];
                self.coverage_error(
                    bin,
                    format!(
                        "transition bins `{}` of coverpoint `{point_name}` {message} (llg limitation)",
                        self.node(bin).name
                    ),
                )
            })?;
            coverpoint.transitions = Some(tables);
        }
        Ok(coverpoint)
    }

    /// One cross (19.6): its crossed coverpoints and its bins' select
    /// expressions in disjunctive form.
    fn cross_table(
        &mut self,
        group_name: &str,
        cross: NodeId,
        info: &CovergroupInfo,
        points: &mut [IrCoverpoint],
        build: &mut CoverageBuild,
    ) -> Result<IrCoverCross, String> {
        let metadata = self
            .db
            .coverage_cross(cross)
            .cloned()
            .ok_or_else(|| self.coverage_error(cross, "cross has no captured structure"))?;
        let cross_name = self.node(cross).name.clone();
        let describe = format!("cross `{cross_name}` in covergroup `{group_name}`");
        if metadata.crosses_cross {
            return Err(self.coverage_error(
                cross,
                format!(
                    "{describe} crosses another cross, which IEEE 1800-2009 does not define (19.6)"
                ),
            ));
        }
        let mut dims = Vec::with_capacity(metadata.points.len());
        for point in &metadata.points {
            let index = info
                .coverpoints
                .iter()
                .position(|candidate| candidate == point)
                .ok_or_else(|| {
                    self.coverage_error(
                        cross,
                        format!("{describe} crosses an item that is not a coverpoint of the covergroup (IEEE 1800-2009 19.6)"),
                    )
                })?;
            if points[index].transitions.is_some() {
                return Err(self.coverage_error(
                    cross,
                    format!(
                        "{describe} crosses coverpoint `{}`, which has transition bins; llg crosses only value bins (llg limitation)",
                        points[index].name
                    ),
                ));
            }
            dims.push(index as u32);
        }
        if dims.len() < 2 {
            return Err(self.coverage_error(
                cross,
                format!("{describe} must cross at least two coverpoints (IEEE 1800-2009 19.6)"),
            ));
        }
        for setter in &metadata.options {
            let (scope, member, value) = self.coverage_option_parts(*setter).ok_or_else(|| {
                self.coverage_error(*setter, "coverage option setter has no option member")
            })?;
            self.check_option(group_name, &scope, &member, value, *setter, false)?;
        }
        let mut table = IrCoverCross {
            name: cross_name,
            points: dims.clone(),
            selects: Vec::new(),
            bins: Vec::with_capacity(metadata.bins.len()),
            items: Vec::new(),
        };
        for bin in metadata.bins.iter().copied() {
            let bin_info = self.db.coverage_bin(bin).cloned().ok_or_else(|| {
                self.coverage_error(bin, "cross bins declaration has no structure")
            })?;
            let bin_name = self.node(bin).name.clone();
            let kind = match bin_info.kind {
                CoverageBinKind::Bins => IrCoverBinKind::Bins,
                CoverageBinKind::Ignore => IrCoverBinKind::Ignore,
                CoverageBinKind::Illegal => IrCoverBinKind::Illegal,
            };
            let select = bin_info
                .select_expr
                .as_ref()
                .filter(|_| !(bin_info.with || bin_info.set || bin_info.array || bin_info.default));
            let Some(select) = select else {
                return Err(self.coverage_error(
                    bin,
                    format!(
                        "bins `{bin_name}` of {describe} must be a select expression; set expressions, `with` clauses, bin arrays and `default` are not IEEE 1800-2009 cross bins (19.6)"
                    ),
                ));
            };
            let boxes = self.cross_select(
                select,
                false,
                &metadata.points,
                &dims,
                points,
                &mut table,
                &describe,
                &bin_name,
                build,
            )?;
            let guard = bin_info.iff.map(|iff| {
                build.guards.push(iff);
                (build.guards.len() - 1) as u32
            });
            table.bins.push(IrCoverCrossBin {
                name: bin_name,
                site: Self::coverage_site(&build.path, self.node(bin)),
                kind,
                guard,
                boxes,
            });
        }
        for dim in &dims {
            points[*dim as usize].crossed = true;
        }
        Ok(table)
    }

    /// A select expression (or its negation) as a union of boxes; negations
    /// reach the conditions by De Morgan's laws.
    #[allow(clippy::too_many_arguments)]
    fn cross_select(
        &mut self,
        select: &CoverageSelect,
        negate: bool,
        cross_points: &[NodeId],
        dims: &[u32],
        points: &[IrCoverpoint],
        table: &mut IrCoverCross,
        describe: &str,
        bin_name: &str,
        build: &mut CoverageBuild,
    ) -> Result<Vec<Vec<u32>>, String> {
        let union = |mut a: Vec<Vec<u32>>, b: Vec<Vec<u32>>| {
            a.extend(b);
            a
        };
        let product = |a: Vec<Vec<u32>>, b: Vec<Vec<u32>>| {
            let mut out = Vec::with_capacity(a.len() * b.len());
            for left in &a {
                for right in &b {
                    let mut both = left.clone();
                    both.extend(right);
                    out.push(both);
                }
            }
            out
        };
        let boxes = match &select.kind {
            CoverageSelectKind::Condition { target, intersect } => {
                let (point_node, decl) = if self.db.coverpoint(*target).is_some() {
                    (*target, None)
                } else if let Some(bin) = self.db.coverage_bin(*target).cloned() {
                    (bin.point, Some(self.coverage_value_decl(bin.point, *target)))
                } else {
                    return Err(self.coverage_error(
                        select.node,
                        format!("binsof in bins `{bin_name}` of {describe} must name a coverpoint or a bin of one (IEEE 1800-2009 19.6)"),
                    ));
                };
                let dim = cross_points
                    .iter()
                    .position(|point| *point == point_node)
                    .ok_or_else(|| {
                        self.coverage_error(
                            select.node,
                            format!(
                                "binsof in bins `{bin_name}` of {describe} names coverpoint `{}`, which the cross does not cross (decision S45-D13)",
                                self.coverpoint_name(point_node)
                            ),
                        )
                    })?;
                let decl = match decl {
                    Some(Some(decl)) => Some(decl),
                    // A transition bin cannot be crossed (checked above).
                    Some(None) => {
                        return Err(self.coverage_error(
                            select.node,
                            format!("binsof in bins `{bin_name}` of {describe} names a transition bin; llg crosses only value bins (llg limitation)"),
                        ))
                    }
                    None => None,
                };
                let point = &points[dims[dim] as usize];
                let mut scratch = IrCoverpoint {
                    name: String::new(),
                    width: point.width,
                    signed: point.signed,
                    automatic: IrCoverAutomatic::None,
                    bins: Vec::new(),
                    items: Vec::new(),
                    transitions: None,
                    crossed: false,
                };
                for item in intersect.iter().copied() {
                    self.coverage_value_item(bin_name, item, false, &mut scratch, build)?;
                }
                let first_item = table.items.len() as u32;
                let n_items = scratch.items.len() as u32;
                table.items.extend(scratch.items);
                table.selects.push(IrCoverSelect {
                    dim: dim as u32,
                    decl,
                    intersect: !intersect.is_empty(),
                    first_item,
                    n_items,
                    negate,
                });
                vec![vec![(table.selects.len() - 1) as u32]]
            }
            CoverageSelectKind::Not(inner) => self.cross_select(
                inner,
                !negate,
                cross_points,
                dims,
                points,
                table,
                describe,
                bin_name,
                build,
            )?,
            CoverageSelectKind::And(left, right) | CoverageSelectKind::Or(left, right) => {
                let left = self.cross_select(
                    left, negate, cross_points, dims, points, table, describe, bin_name, build,
                )?;
                let right = self.cross_select(
                    right, negate, cross_points, dims, points, table, describe, bin_name, build,
                )?;
                let and = matches!(select.kind, CoverageSelectKind::And(..)) != negate;
                if and {
                    if (left.len() as u64) * (right.len() as u64) > MAX_SELECT_BOXES {
                        return Err(self.coverage_error(
                            select.node,
                            format!("the select expression of bins `{bin_name}` of {describe} expands to more than {MAX_SELECT_BOXES} terms (llg limitation)"),
                        ));
                    }
                    product(left, right)
                } else {
                    union(left, right)
                }
            }
            CoverageSelectKind::Other => {
                return Err(self.coverage_error(
                    select.node,
                    format!("the select expression of bins `{bin_name}` of {describe} is not IEEE 1800-2009 syntax (19.6)"),
                ))
            }
        };
        if boxes.len() as u64 > MAX_SELECT_BOXES {
            return Err(self.coverage_error(
                select.node,
                format!("the select expression of bins `{bin_name}` of {describe} expands to more than {MAX_SELECT_BOXES} terms (llg limitation)"),
            ));
        }
        Ok(boxes)
    }

    /// The index of a bins declaration among its coverpoint's value bins
    /// ([`IrCoverpoint::bins`]), or `None` for a transition bin.
    fn coverage_value_decl(&self, point: NodeId, bin: NodeId) -> Option<u32> {
        let info = self.db.coverpoint(point)?;
        let mut index = 0u32;
        for candidate in &info.bins {
            let transition = self
                .db
                .coverage_bin(*candidate)
                .is_some_and(|metadata| metadata.transition || metadata.default_sequence);
            if *candidate == bin {
                return (!transition).then_some(index);
            }
            if !transition {
                index += 1;
            }
        }
        None
    }

    /// One transition bins declaration (19.5.1): its sets as key ranges and
    /// repetitions, checked against the clause's restrictions.
    fn coverage_transition_bin(
        &mut self,
        group_name: &str,
        point: NodeId,
        bin: NodeId,
        width: u32,
        signed: bool,
        build: &mut CoverageBuild,
    ) -> Result<transitions::BinSpec, String> {
        let metadata = self
            .db
            .coverage_bin(bin)
            .cloned()
            .ok_or_else(|| self.coverage_error(bin, "bins declaration has no structure"))?;
        let bin_name = self.node(bin).name.clone();
        let describe = format!(
            "`{bin_name}` of coverpoint `{}` in covergroup `{group_name}`",
            self.coverpoint_name(point)
        );
        if metadata.with || metadata.set || metadata.select {
            return Err(self.coverage_error(
                bin,
                format!(
                    "bins {describe} use a `with` clause, a set expression or a select expression, which IEEE 1800-2009 does not define for coverpoint bins"
                ),
            ));
        }
        let kind = match (metadata.kind, metadata.default_sequence) {
            (CoverageBinKind::Bins, false) => IrCoverBinKind::Bins,
            (CoverageBinKind::Ignore, false) => IrCoverBinKind::Ignore,
            (CoverageBinKind::Illegal, false) => IrCoverBinKind::Illegal,
            (CoverageBinKind::Bins, true) => IrCoverBinKind::Default,
            (CoverageBinKind::Illegal, true) => IrCoverBinKind::IllegalDefault,
            (CoverageBinKind::Ignore, true) => {
                return Err(self.coverage_error(
                    bin,
                    format!("ignore_bins {describe} cannot specify `default sequence` (IEEE 1800-2009 19.5)"),
                ))
            }
        };
        let array = metadata.array && kind == IrCoverBinKind::Bins && !metadata.default_sequence;
        if metadata.array && metadata.count.is_some() && !metadata.default_sequence {
            return Err(self.coverage_error(
                bin,
                format!(
                    "transition bins {describe} cannot have a number of bins; only `[]` creates one bin per sequence (IEEE 1800-2009 19.5.1)"
                ),
            ));
        }
        let guard = metadata.iff.map(|iff| {
            build.guards.push(iff);
            (build.guards.len() - 1) as u32
        });
        let mut sets = Vec::with_capacity(metadata.transitions.len());
        for set in &metadata.transitions {
            let mut steps = Vec::with_capacity(set.len());
            for range in set {
                let (repeat, lo, hi) = match range.repeat {
                    CoverageRepeat::None => (IrCoverRepeat::Consecutive, 1, 1),
                    repeat => {
                        let repeat = match repeat {
                            CoverageRepeat::GoTo => IrCoverRepeat::GoTo,
                            CoverageRepeat::Nonconsecutive => IrCoverRepeat::Nonconsecutive,
                            _ => IrCoverRepeat::Consecutive,
                        };
                        let from = range.repeat_from.ok_or_else(|| {
                            self.coverage_error(range.node, "transition repetition has no count")
                        })?;
                        let lo = self.coverage_repeat_count(from, &describe)?;
                        let hi = match range.repeat_to {
                            Some(to) => self.coverage_repeat_count(to, &describe)?,
                            None => lo,
                        };
                        if hi < lo {
                            return Err(self.coverage_error(
                                range.node,
                                format!(
                                    "the repetition range of transition bins {describe} is empty"
                                ),
                            ));
                        }
                        (repeat, lo, hi)
                    }
                };
                let bounded = repeat == IrCoverRepeat::Consecutive;
                if !bounded && array {
                    return Err(self.coverage_error(
                        range.node,
                        format!(
                            "transition bins {describe} with `[]` cannot use goto or nonconsecutive repetition, whose sequences have no fixed length (IEEE 1800-2009 19.5.1)"
                        ),
                    ));
                }
                if !bounded && kind != IrCoverBinKind::Bins {
                    return Err(self.coverage_error(
                        range.node,
                        format!(
                            "ignore_bins and illegal_bins {describe} cannot use goto or nonconsecutive repetition, whose sequences have no fixed length (IEEE 1800-2009 19.5.4, 19.5.5)"
                        ),
                    ));
                }
                let mut scratch = IrCoverpoint {
                    name: String::new(),
                    width,
                    signed,
                    automatic: IrCoverAutomatic::None,
                    bins: Vec::new(),
                    items: Vec::new(),
                    transitions: None,
                    crossed: false,
                };
                let max_key = if width == 64 {
                    u64::MAX
                } else {
                    (1u64 << width) - 1
                };
                let mut ranges = Vec::new();
                for item in range.items.iter().copied() {
                    scratch.items.clear();
                    self.coverage_value_item(
                        &bin_name,
                        item,
                        metadata.wildcard,
                        &mut scratch,
                        build,
                    )?;
                    for value in &scratch.items {
                        let key = |bound: IrCoverBound| match bound {
                            IrCoverBound::Key(key) => Some(key),
                            IrCoverBound::Min => Some(0),
                            IrCoverBound::Max => Some(max_key),
                            IrCoverBound::Slot(_) => None,
                        };
                        match (key(value.lo), key(value.hi)) {
                            (Some(lo), Some(hi)) => ranges.push((lo, hi)),
                            _ => {
                                return Err(self.coverage_error(
                                    item,
                                    format!(
                                        "transition bins {describe} read a covergroup argument; llg supports only constant transition values (llg limitation)"
                                    ),
                                ))
                            }
                        }
                    }
                }
                steps.push(transitions::StepSpec {
                    ranges,
                    repeat,
                    lo,
                    hi,
                });
            }
            // 19.5.1: "Transition bin specifications of length 0 shall be
            // illegal": a single range list, alone or with a consecutive
            // repetition evaluating to 1. Goto and nonconsecutive
            // repetitions, and a consecutive range that may exceed 1, are
            // sequences the text does not name; one sample can complete
            // them (decision S45-D19).
            if steps.len() == 1 && steps[0].repeat == IrCoverRepeat::Consecutive && steps[0].hi == 1
            {
                let node = set.first().map_or(bin, |range| range.node);
                return Err(self.coverage_error(
                    node,
                    format!(
                        "transition bins {describe} specify a transition of length 0: a single range list, alone or with a consecutive repetition that evaluates to 1 (IEEE 1800-2009 19.5.1)"
                    ),
                ));
            }
            sets.push(steps);
        }
        Ok(transitions::BinSpec {
            name: bin_name,
            site: Self::coverage_site(&build.path, self.node(bin)),
            kind,
            array,
            guard,
            sets,
        })
    }

    /// One bound of a transition repetition: a positive constant.
    fn coverage_repeat_count(&self, node: NodeId, describe: &str) -> Result<u32, String> {
        let value = self.eval_bits(node).ok().and_then(|value| {
            if value.signed {
                value.to_i128()
            } else {
                value.to_u128().and_then(|value| i128::try_from(value).ok())
            }
        });
        match value.and_then(|value| u32::try_from(value).ok()) {
            Some(count) if count > 0 => Ok(count),
            _ => Err(self.coverage_error(
                node,
                format!(
                    "the repetition count of transition bins {describe} must be a positive constant"
                ),
            )),
        }
    }

    /// One bins declaration of a coverpoint (19.5, 19.5.3-19.5.6).
    fn coverage_value_bin(
        &mut self,
        group_name: &str,
        point: NodeId,
        bin: NodeId,
        coverpoint: &mut IrCoverpoint,
        build: &mut CoverageBuild,
    ) -> Result<(), String> {
        let metadata = self
            .db
            .coverage_bin(bin)
            .cloned()
            .ok_or_else(|| self.coverage_error(bin, "bins declaration has no structure"))?;
        let bin_name = self.node(bin).name.clone();
        let describe = |cg: &Codegen<'_>| {
            format!(
                "`{bin_name}` of coverpoint `{}` in covergroup `{group_name}`",
                cg.coverpoint_name(point)
            )
        };
        if metadata.with || metadata.set || metadata.select {
            return Err(self.coverage_error(
                bin,
                format!(
                    "bins {} use a `with` clause, a set expression or a select expression, which IEEE 1800-2009 does not define for coverpoint bins",
                    describe(self)
                ),
            ));
        }
        let kind = match (metadata.kind, metadata.default) {
            (CoverageBinKind::Bins, false) => IrCoverBinKind::Bins,
            (CoverageBinKind::Ignore, false) => IrCoverBinKind::Ignore,
            (CoverageBinKind::Illegal, false) => IrCoverBinKind::Illegal,
            (CoverageBinKind::Bins, true) => IrCoverBinKind::Default,
            (CoverageBinKind::Illegal, true) => IrCoverBinKind::IllegalDefault,
            (CoverageBinKind::Ignore, true) => {
                return Err(self.coverage_error(
                    bin,
                    format!(
                        "ignore_bins {} cannot specify `default` (IEEE 1800-2009 19.5)",
                        describe(self)
                    ),
                ))
            }
        };
        let shape = if !metadata.array || metadata.default || kind != IrCoverBinKind::Bins {
            IrCoverBinShape::Scalar
        } else {
            match metadata.count {
                None => IrCoverBinShape::PerValue,
                Some(count) => IrCoverBinShape::Count(self.coverage_bin_count(
                    count,
                    &describe(self),
                    build,
                )?),
            }
        };
        let guard = metadata.iff.map(|iff| {
            build.guards.push(iff);
            (build.guards.len() - 1) as u32
        });
        let first_item = coverpoint.items.len() as u32;
        for item in metadata.values.iter().copied() {
            self.coverage_value_item(&bin_name, item, metadata.wildcard, coverpoint, build)?;
        }
        let n_items = coverpoint.items.len() as u32 - first_item;
        coverpoint.bins.push(IrCoverBin {
            name: bin_name,
            site: Self::coverage_site(&build.path, self.node(bin)),
            kind,
            shape,
            guard,
            first_item,
            n_items,
        });
        Ok(())
    }

    /// `path:line:col` of a declaration, for run-time messages.
    fn coverage_site(path: &str, node: &crate::core::db::Node) -> String {
        if node.line == 0 {
            path.to_owned()
        } else {
            format!("{path}:{}:{}", node.line, node.col)
        }
    }

    /// The `[N]` count of a bins array: a positive constant, or an argument
    /// value checked when the instance is ready.
    fn coverage_bin_count(
        &mut self,
        count: NodeId,
        describe: &str,
        build: &mut CoverageBuild,
    ) -> Result<IrCoverCount, String> {
        match self.eval_bits(count) {
            Ok(value) => {
                let number = if value.signed {
                    value.to_i128()
                } else {
                    value.to_u128().and_then(|value| i128::try_from(value).ok())
                };
                match number.and_then(|number| u64::try_from(number).ok()) {
                    Some(number) if number > 0 => Ok(IrCoverCount::Const(number)),
                    _ => Err(self.coverage_error(
                        count,
                        format!("the number of bins of {describe} must be a positive value"),
                    )),
                }
            }
            Err(_) => Ok(IrCoverCount::Slot(build.slot(count))),
        }
    }

    /// One open-range-list item of a value bin: constant items resolve here
    /// by 19.5.6 with its warnings; items that read covergroup arguments
    /// become slots resolved when each instance is ready.
    fn coverage_value_item(
        &mut self,
        bin_name: &str,
        item: NodeId,
        wildcard: bool,
        coverpoint: &mut IrCoverpoint,
        build: &mut CoverageBuild,
    ) -> Result<(), String> {
        let (width, signed) = (coverpoint.width, coverpoint.signed);
        let (low_node, high_node) = match self.kind(item) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::List,
                operands,
                ..
            }) if operands.len() == 2 => (operands[0], operands[1]),
            _ => (item, item),
        };
        let range = low_node != high_node;
        // 19.5.3: a wildcard singleton is the set of values its X, Z and ?
        // bits leave open.
        if wildcard && !range {
            let node = self.coverage_bin_operand(item, width, signed);
            let value = self.eval_bits(node).map_err(|_| {
                self.coverage_error(
                    item,
                    format!("wildcard bins `{bin_name}` need constant values"),
                )
            })?;
            for (lo, hi) in self.wildcard_ranges(bin_name, item, &value, width, signed)? {
                coverpoint.items.push(IrCoverItem {
                    lo: IrCoverBound::Key(lo),
                    hi: IrCoverBound::Key(hi),
                    site: None,
                });
            }
            return Ok(());
        }
        let (min, max) = if signed {
            (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
        } else {
            (0, (1i128 << width) - 1)
        };
        let low = self.coverage_bound(low_node, width, signed, wildcard, false);
        let high = self.coverage_bound(high_node, width, signed, wildcard, true);
        if matches!(low, CoverageBound::Unknown) || matches!(high, CoverageBound::Unknown) {
            self.coverage_warning(
                item,
                format!("bins `{bin_name}` value has X or Z bits and is ignored (IEEE 1800-2009 19.5.6)"),
            );
            return Ok(());
        }
        if let (Some(low), Some(high)) = (low.number(min), high.number(max)) {
            let (low_in, high_in) = (low.max(min), high.min(max));
            if low > high || low_in > high_in {
                self.coverage_warning(
                    item,
                    format!("bins `{bin_name}` value is outside the coverpoint type and is ignored (IEEE 1800-2009 19.5.6)"),
                );
                return Ok(());
            }
            if low_in != low || high_in != high {
                self.coverage_warning(
                    item,
                    format!("bins `{bin_name}` range is limited to the values of the coverpoint type (IEEE 1800-2009 19.5.6)"),
                );
            }
            coverpoint.items.push(IrCoverItem {
                lo: IrCoverBound::Key(Self::coverage_key(low_in, width, signed)),
                hi: IrCoverBound::Key(Self::coverage_key(high_in, width, signed)),
                site: None,
            });
            return Ok(());
        }
        if wildcard {
            return Err(self.coverage_error(
                item,
                format!("wildcard bins `{bin_name}` need constant values"),
            ));
        }
        // 19.5: bins values may be non-ref arguments of the covergroup.
        let mut bound =
            |cg: &mut Codegen<'_>, node: NodeId, value: CoverageBound, upper: bool| match value {
                CoverageBound::Dollar if upper => IrCoverBound::Max,
                CoverageBound::Dollar => IrCoverBound::Min,
                _ => IrCoverBound::Slot(build.slot(cg.coverage_bin_operand(node, width, signed))),
            };
        let lo = bound(self, low_node, low, false);
        let hi = if range {
            bound(self, high_node, high, true)
        } else {
            lo
        };
        coverpoint.items.push(IrCoverItem {
            lo,
            hi,
            site: Some(Self::coverage_site(&build.path, self.node(item))),
        });
        Ok(())
    }

    /// The bins value before the frontend's conversion to the coverpoint
    /// type: 19.5.6 resolves the unconverted value against that type.
    fn coverage_bin_operand(&self, node: NodeId, width: u32, signed: bool) -> NodeId {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. })
                if ty.width == Some(width) && ty.signed == signed =>
            {
                *operand
            }
            _ => node,
        }
    }

    /// One bins bound as a constant, `$`, or a value read at `new`. A
    /// wildcard range bound stands for the smallest (low) or largest
    /// (high) value its open bits allow.
    fn coverage_bound(
        &self,
        node: NodeId,
        width: u32,
        signed: bool,
        wildcard: bool,
        upper: bool,
    ) -> CoverageBound {
        let node = self.coverage_bin_operand(node, width, signed);
        if matches!(self.kind(node), NodeKind::Expr(ExprKind::Unbounded)) {
            return CoverageBound::Dollar;
        }
        let Ok(mut value) = self.eval_bits(node) else {
            return CoverageBound::Instance;
        };
        if wildcard {
            let fill = if upper {
                crate::core::elab::Bit::One
            } else {
                crate::core::elab::Bit::Zero
            };
            for bit in &mut value.bits {
                if matches!(bit, crate::core::elab::Bit::X | crate::core::elab::Bit::Z) {
                    *bit = fill;
                }
            }
        }
        if value.is_unknown() {
            return CoverageBound::Unknown;
        }
        let number = if value.signed {
            value.to_i128()
        } else {
            value.to_u128().and_then(|value| i128::try_from(value).ok())
        };
        match number {
            Some(number) => CoverageBound::Number(number),
            // Wider than any coverpoint value: outside the type.
            None if value.signed && value.bits.first() == Some(&crate::core::elab::Bit::One) => {
                CoverageBound::Number(i128::MIN)
            }
            None => CoverageBound::Number(i128::MAX),
        }
    }

    /// The key ranges of a wildcard value (19.5.3, 19.5.6): bits that are X,
    /// Z or ? match 0 and 1. A pattern whose open bits above its lowest
    /// fixed bit need more than [`WILDCARD_RANGE_LIMIT`] ranges is rejected.
    fn wildcard_ranges(
        &mut self,
        bin_name: &str,
        item: NodeId,
        value: &crate::core::elab::Value,
        width: u32,
        signed: bool,
    ) -> Result<Vec<(u64, u64)>, String> {
        use crate::core::elab::Bit;
        let mask = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let source = value.width();
        let extension = if value.signed && source > 0 {
            value.bit_lsb(source - 1)
        } else {
            Bit::Zero
        };
        let (mut fixed, mut care) = (0u64, 0u64);
        let mut limited = false;
        for index in 0..source.max(width as usize) {
            let bit = if index < source {
                value.bit_lsb(index)
            } else {
                extension
            };
            if index >= width as usize {
                // A value bit beyond the coverpoint: a fixed 1 (0 for a
                // negative signed value) leaves the type; an open bit
                // narrows the pattern to the representable values.
                match bit {
                    Bit::One => {
                        self.coverage_warning(
                            item,
                            format!("bins `{bin_name}` value is outside the coverpoint type and is ignored (IEEE 1800-2009 19.5.6)"),
                        );
                        return Ok(Vec::new());
                    }
                    Bit::X | Bit::Z => limited = true,
                    Bit::Zero => {}
                }
                continue;
            }
            match bit {
                Bit::One => {
                    fixed |= 1 << index;
                    care |= 1 << index;
                }
                Bit::Zero => care |= 1 << index,
                Bit::X | Bit::Z => {}
            }
        }
        if limited {
            self.coverage_warning(
                item,
                format!("bins `{bin_name}` range is limited to the values of the coverpoint type (IEEE 1800-2009 19.5.6)"),
            );
        }
        let open = !care & mask;
        if open == mask {
            return Ok(vec![(0, mask)]);
        }
        // The open bits below the lowest fixed bit form one aligned block.
        let low_block = (1u64 << care.trailing_zeros()) - 1;
        let high_open = open & !low_block;
        if high_open.count_ones() > WILDCARD_RANGE_LIMIT.trailing_zeros() {
            return Err(self.coverage_error(
                item,
                format!(
                    "wildcard bins `{bin_name}` value matches more than {WILDCARD_RANGE_LIMIT} separate value ranges, which llg does not support"
                ),
            ));
        }
        // Flipping the sign bit, which lies above the block, keeps each
        // block contiguous in key order.
        let sign = if signed { 1u64 << (width - 1) } else { 0 };
        let mut ranges = Vec::new();
        // Every assignment of the open bits above the lowest fixed bit.
        let mut subset = 0u64;
        loop {
            let base = fixed | subset;
            ranges.push((base ^ sign, (base | low_block) ^ sign));
            subset = subset.wrapping_sub(high_open) & high_open;
            if subset == 0 {
                break;
            }
        }
        ranges.sort_unstable();
        Ok(ranges)
    }

    fn coverpoint_name(&self, point: NodeId) -> String {
        let name = &self.node(point).name;
        if !name.is_empty() {
            return name.clone();
        }
        let node = self.node(point);
        format!("<coverpoint at {}:{}>", node.line, node.col)
    }

    fn coverage_key(value: i128, width: u32, signed: bool) -> u64 {
        let modulus = 1i128 << width;
        let raw = value.rem_euclid(modulus) as u64;
        if signed {
            raw ^ (1u64 << (width - 1))
        } else {
            raw
        }
    }

    fn enum_coverpoint_keys(
        &self,
        point: NodeId,
        info: &CoverpointInfo,
        width: u32,
        signed: bool,
    ) -> Result<Option<Vec<u64>>, String> {
        // An enum coverpoint has one bin per named value (19.5.2).
        let Some(metadata) = info
            .type_id
            .and_then(|type_id| self.db.enum_type_metadata(type_id))
            .or_else(|| self.enum_metadata_for_expr(info.expr))
            .or_else(|| match self.kind(info.expr) {
                // The frontend converts the expression to the coverpoint's
                // integral type; an enum expression keeps its member bins.
                NodeKind::Expr(ExprKind::Cast { operand, ty, .. })
                    if ty.width == Some(width) && ty.signed == signed =>
                {
                    self.enum_metadata_for_expr(*operand)
                }
                _ => None,
            })
        else {
            return Ok(None);
        };
        let mut keys = Vec::with_capacity(metadata.members.len());
        for member in &metadata.members {
            let crate::core::elab::Val::Bits(bits) = &member.value else {
                return Err(self.coverage_error(point, "enum member has no integral value"));
            };
            let value = if bits.signed {
                bits.to_i128()
            } else {
                bits.to_u128().and_then(|value| i128::try_from(value).ok())
            }
            .ok_or_else(|| {
                self.coverage_error(point, "enum member value is not a known integer")
            })?;
            keys.push(Self::coverage_key(value, width, signed));
        }
        keys.sort_unstable();
        keys.dedup();
        Ok(Some(keys))
    }

    fn coverage_warning(&mut self, node: NodeId, message: String) {
        let warning = self.coverage_error(node, message);
        self.warnings.push(warning);
    }

    /// Lower every constructed covergroup's `sample` body and its sampling
    /// event. Runs after subroutine prototypes exist and before any body or
    /// process that constructs an instance is lowered.
    pub(in super::super) fn emit_covergroups(&mut self) -> Result<(), String> {
        let mut groups = self.covergroups.keys().copied().collect::<Vec<_>>();
        groups.sort_by_key(|group| group.index());
        for group in groups {
            self.emit_covergroup_sample(group)?;
            self.emit_covergroup_event(group)?;
        }
        Ok(())
    }

    fn emit_covergroup_sample(&mut self, group: NodeId) -> Result<(), String> {
        // A covergroup of a checker reads checker formals as the sampled
        // values of their actual arguments (IEEE 1800-2009 17.3, 17.6).
        let checker = self.db.has_checker_instances()
            && self
                .db
                .checker_instance(self.covergroup_inst(group))
                .is_some();
        let previous = std::mem::replace(&mut self.checker_sampling, checker);
        let result = self.emit_covergroup_sample_body(group);
        self.checker_sampling = previous;
        result
    }

    fn emit_covergroup_sample_body(&mut self, group: NodeId) -> Result<(), String> {
        let lowering = self.covergroups[&group].clone();
        let info = self
            .db
            .covergroup(group)
            .cloned()
            .ok_or("covergroup vanished")?;
        let path = self.covergroup_path(group);
        let inst = self.covergroup_inst(group);
        let this = IrChandleExpr::LocalRead("_this".to_owned());
        self.cur_fn_ir = Some(lowering.sample);
        let record = IrChandleExpr::LocalRead(self.native_access_symbol(
            this.clone(),
            IrNativeAccessKind::ClassField {
                class: lowering.class,
                field: lowering.record_field,
            },
        ));
        let owner = lowering.owner_field.map(|field| {
            IrChandleExpr::LocalRead(self.native_access_symbol(
                this.clone(),
                IrNativeAccessKind::ClassField {
                    class: lowering.class,
                    field,
                },
            ))
        });
        let bind = lowering.bind_field.map(|field| {
            self.native_access_symbol(
                this.clone(),
                IrNativeAccessKind::ClassField {
                    class: lowering.class,
                    field,
                },
            )
        });
        let mut arg_ir = HashMap::new();
        let mut arg_read = HashMap::new();
        for (index, formal) in info.sample_formals.iter().enumerate() {
            let function = &self.model.funcs[lowering.sample];
            let ir_formal = &function.formals[index];
            arg_ir.insert(
                *formal,
                formal_read_expr(index, ir_formal.width, ir_formal.signed),
            );
            arg_read.insert(
                *formal,
                ArgMap {
                    width: ir_formal.width,
                    signed: ir_formal.signed,
                    two_state: ir_formal.two_state,
                },
            );
        }
        let mut func = Self::coverage_func_ctx(
            format!("{}::sample", self.covergroup_name(group)),
            false,
            owner,
        );
        func.arg_read = arg_read;
        func.arg_ir = arg_ir;
        let saved_scope = self.coverage_scope.replace((lowering.class, this.clone()));
        let result = (|| -> Result<(Vec<IrStmt>, Vec<IrPreFn>), String> {
            let mut ctx = EmitCtx::new(self, path.clone(), inst, "depth + 1", Some(func), false);
            let mut variants = Vec::with_capacity(lowering.variants.len());
            for binding in &lowering.variants {
                ctx.cg.coverage_ref_actuals = binding.clone();
                let mut samples = Vec::with_capacity(info.coverpoints.len());
                for (index, point) in info.coverpoints.iter().copied().enumerate() {
                    let metadata = ctx
                        .cg
                        .db
                        .coverpoint(point)
                        .cloned()
                        .ok_or("coverpoint vanished")?;
                    let width = ctx.cg.model.covergroups[lowering.ir].points[index].width;
                    let signed = ctx.cg.model.covergroups[lowering.ir].points[index].signed;
                    let located = |cg: &Codegen<'_>, node: NodeId, error: String| {
                        cg.coverage_error(
                            node,
                            format!(
                                "coverpoint `{}` cannot be sampled: {error}",
                                cg.coverpoint_name(point)
                            ),
                        )
                    };
                    let value = ctx
                        .cg
                        .lower_expr(&path, metadata.expr)
                        .and_then(|value| ir_to_vector(value, width, signed))
                        .map_err(|error| located(ctx.cg, metadata.expr, error))?;
                    let iff = metadata
                        .iff
                        .map(|iff| ctx.cg.lower_boolean_expr(&path, iff))
                        .transpose()
                        .map_err(|error| located(ctx.cg, metadata.iff.unwrap_or(point), error))?;
                    let history = ctx.cg.model.covergroups[lowering.ir].points[index]
                        .transitions
                        .is_some();
                    samples.push(IrCoverageSample {
                        point: index as u32,
                        iff,
                        value,
                        history,
                    });
                }
                let mut crosses = Vec::with_capacity(info.crosses.len());
                for (index, cross) in info.crosses.iter().copied().enumerate() {
                    let iff = ctx
                        .cg
                        .db
                        .coverage_cross(cross)
                        .and_then(|metadata| metadata.iff);
                    let iff = iff
                        .map(|iff| ctx.cg.lower_boolean_expr(&path, iff))
                        .transpose()
                        .map_err(|error| {
                            ctx.cg.coverage_error(
                                iff.unwrap_or(cross),
                                format!(
                                    "cross `{}` cannot be sampled: {error}",
                                    ctx.cg.node(cross).name
                                ),
                            )
                        })?;
                    crosses.push(IrCoverageCrossSample {
                        cross: index as u32,
                        iff,
                    });
                }
                let mut guards = Vec::with_capacity(lowering.build.guards.len());
                for guard in lowering.build.guards.iter().copied() {
                    let lowered = ctx.cg.lower_boolean_expr(&path, guard).map_err(|error| {
                        ctx.cg
                            .coverage_error(guard, format!("bins guard cannot be sampled: {error}"))
                    })?;
                    guards.push(lowered);
                }
                variants.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Sample {
                    record: record.clone(),
                    guards,
                    points: samples,
                    crosses,
                })));
            }
            ctx.cg.coverage_ref_actuals.clear();
            let body = match &bind {
                None => variants,
                Some(bind) => {
                    // One statically lowered sample per binding variant.
                    let selector =
                        IrExpr::new(IrExprKind::LocalRead(bind.clone()), 32, false, None);
                    let mut chain: Option<Vec<IrStmt>> = None;
                    for (variant, statement) in variants.into_iter().enumerate().rev() {
                        let condition = IrExpr::new(
                            IrExprKind::Bin {
                                op: IrBinOp::Eq,
                                a: Box::new(selector.clone()),
                                b: Box::new(lhs_integer_expr(variant as i128)),
                            },
                            1,
                            false,
                            None,
                        );
                        chain = Some(vec![IrStmt::If {
                            cond: condition,
                            then_: vec![statement],
                            els: chain,
                            check: IrUniquePriorityCheck::None,
                        }]);
                    }
                    chain.unwrap_or_default()
                }
            };
            Ok((body, std::mem::take(&mut ctx.pre_fns)))
        })();
        self.coverage_scope = saved_scope;
        self.func = None;
        self.cur_fn_ir = None;
        let (body, pre_fns) = result?;
        let function = &mut self.model.funcs[lowering.sample];
        function.body = body;
        function.pre_fns = pre_fns;
        Ok(())
    }

    /// A [`FuncCtx`] for code that runs on behalf of one covergroup object;
    /// owner properties read through `owner`.
    fn coverage_func_ctx(name: String, is_task: bool, owner: Option<IrChandleExpr>) -> FuncCtx {
        FuncCtx {
            name,
            is_task,
            class_receiver: owner,
            ret: None,
            arg_read: HashMap::new(),
            arg_ir: HashMap::new(),
            event_args: HashMap::new(),
            arg_dependencies: HashMap::new(),
            arg_write: HashMap::new(),
            arg_lhs: HashMap::new(),
            const_refs: HashSet::new(),
            const_ref_lhs: HashMap::new(),
            persistent: HashMap::new(),
            chandle_read: HashMap::new(),
            chandle_write: HashMap::new(),
            process_read: HashMap::new(),
            process_write: HashMap::new(),
            string_read: HashMap::new(),
            string_write: HashMap::new(),
            string_addr: HashMap::new(),
            locals: HashMap::new(),
            ret_node: None,
            def_node: None,
        }
    }

    /// A clocking event that reads one instance's storage (a covergroup
    /// argument or an owner property, 19.3) becomes a waiter per instance:
    /// a captured helper of the sample method whose only capture is the
    /// covergroup object, started by every `new` as a runtime-owned process
    /// (decision S44-D5) whose first turn, arming the event, runs inside
    /// `new` (S44-D6). It loops on the event and samples its instance;
    /// the capture keeps the instance, and through it the owner and the
    /// arguments, reachable for the rest of the run (S44-D3).
    fn emit_covergroup_waiter(
        &mut self,
        group: NodeId,
        timing: NodeId,
        specs: &[EventSpec],
    ) -> Result<(), String> {
        let lowering = self.covergroups[&group].clone();
        let name = self.covergroup_name(group);
        let path = self.covergroup_path(group);
        let inst = self.covergroup_inst(group);
        let frame = self.new_frame_id()?;
        let storage = StorageRef::new(
            frame,
            0,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        )
        .with_kind(StorageKind::Opaque);
        let object = IrChandleExpr::LocalRead(Codegen::capture_local_name(storage));
        self.cur_fn_ir = Some(lowering.sample);
        let field = |cg: &mut Codegen<'_>, field: usize| {
            cg.native_access_symbol(
                object.clone(),
                IrNativeAccessKind::ClassField {
                    class: lowering.class,
                    field,
                },
            )
        };
        let record = IrChandleExpr::LocalRead(field(self, lowering.record_field));
        let owner = lowering
            .owner_field
            .map(|owner| IrChandleExpr::LocalRead(field(self, owner)));
        let bind = lowering.bind_field.map(|bind| field(self, bind));
        let func = Self::coverage_func_ctx(format!("{name}::sample event"), true, owner);
        let saved_scope = self
            .coverage_scope
            .replace((lowering.class, object.clone()));
        let result = (|| -> Result<(Vec<Vec<IrStmt>>, Vec<IrPreFn>), String> {
            let mut ctx = EmitCtx::new(self, path.clone(), inst, "0", Some(func), false);
            let mut waits = Vec::with_capacity(lowering.variants.len());
            for binding in &lowering.variants {
                ctx.cg.coverage_ref_actuals = binding.clone();
                let wait = ctx.lower_sampling_event(timing, specs);
                ctx.cg.coverage_ref_actuals.clear();
                waits.push(wait?);
            }
            Ok((waits, std::mem::take(&mut ctx.pre_fns)))
        })();
        self.coverage_scope = saved_scope;
        self.func = None;
        self.cur_fn_ir = None;
        let (waits, mut pre_fns) = result.map_err(|error| {
            self.coverage_error(
                timing,
                format!("sampling event of covergroup `{name}`: {error}"),
            )
        })?;
        let mut body = match &bind {
            None => waits.into_iter().next().unwrap_or_default(),
            Some(bind) => {
                // The `ref` binding variant of the instance selects the
                // variables its event waits on.
                let selector = IrExpr::new(IrExprKind::LocalRead(bind.clone()), 32, false, None);
                let mut chain: Option<Vec<IrStmt>> = None;
                for (variant, wait) in waits.into_iter().enumerate().rev() {
                    let condition = IrExpr::new(
                        IrExprKind::Bin {
                            op: IrBinOp::Eq,
                            a: Box::new(selector.clone()),
                            b: Box::new(lhs_integer_expr(variant as i128)),
                        },
                        1,
                        false,
                        None,
                    );
                    chain = Some(vec![IrStmt::If {
                        cond: condition,
                        then_: wait,
                        els: chain,
                        check: IrUniquePriorityCheck::None,
                    }]);
                }
                chain.unwrap_or_default()
            }
        };
        body.push(IrStmt::Coverage(Box::new(IrCoverageStmt::SampleInstance {
            group: lowering.ir,
            record,
            object,
            function: lowering.sample,
            depth: parse_depth("0"),
            strobe: lowering.strobe,
        })));
        let c_name = self.new_fn_name(&path, &format!("cov_{}_waiter", ident(&name)));
        // The publisher (`Watch`) fills slot 0 with the new object; the
        // initializer only types the slot for the helper's prologue.
        let captures = vec![IrCapture::new(
            storage,
            IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::HandleCapture(
                    IrChandleExpr::LocalRead("_this".to_owned()),
                ))),
                1,
                false,
                None,
            ),
        )];
        pre_fns.push(IrPreFn::CapturedBranch {
            c_name: c_name.clone(),
            frame,
            captures,
            body: vec![IrStmt::Forever { body }],
        });
        self.model.funcs[lowering.sample].pre_fns.extend(pre_fns);
        if let Some(lowering) = self.covergroups.get_mut(&group) {
            lowering.waiter = Some(c_name);
        }
        Ok(())
    }

    /// Whether an event expression reads storage of one instance (a
    /// covergroup argument or a property of the owning object).
    fn coverage_event_reads_instance(&self, info: &CovergroupInfo, node: NodeId) -> Option<NodeId> {
        let formals = info
            .formals
            .iter()
            .chain(&info.sample_formals)
            .copied()
            .collect::<HashSet<_>>();
        let mut pending = vec![node];
        while let Some(id) = pending.pop() {
            let targets: Vec<NodeId> = match self.kind(id) {
                NodeKind::Expr(ExprKind::Ref { target }) => target.iter().copied().collect(),
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    refs.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            };
            if let Some(target) = targets
                .into_iter()
                .find(|target| formals.contains(target) || self.class_fields.contains_key(target))
            {
                return Some(target);
            }
            pending.extend(self.node(id).children.iter().copied());
        }
        None
    }

    fn emit_covergroup_event(&mut self, group: NodeId) -> Result<(), String> {
        let lowering = self.covergroups[&group].clone();
        let info = self
            .db
            .covergroup(group)
            .cloned()
            .ok_or("covergroup vanished")?;
        let name = self.covergroup_name(group);
        match &info.event {
            None => Ok(()),
            Some(CoverageEvent::Block { events, .. }) => {
                for event in events {
                    let target = self
                        .named_block_statement(event.target)
                        .unwrap_or(event.target);
                    if target == event.target
                        && !matches!(self.kind(event.target), NodeKind::FuncTask { .. })
                    {
                        return Err(self.coverage_error(
                            group,
                            format!(
                                "block event of covergroup `{name}` names `{}`, which is not a named block, task, function or method",
                                self.node(event.target).name
                            ),
                        ));
                    }
                    self.coverage_block_events
                        .entry(target)
                        .or_default()
                        .push((group, event.begin));
                }
                Ok(())
            }
            Some(CoverageEvent::Clocking { timing, specs, .. }) => {
                let mut nodes = Vec::new();
                for spec in specs {
                    spec.referenced_nodes(&mut nodes);
                }
                if lowering.instance_event {
                    return self.emit_covergroup_waiter(group, *timing, specs);
                }
                let path = self.covergroup_path(group);
                let inst = self.covergroup_inst(group);
                let process_name = self.new_fn_name(&path, &format!("cov_{}", ident(&name)));
                let (wait, pre_fns) = {
                    let mut ctx = EmitCtx::new(self, path.clone(), inst, "0", None, false);
                    let wait = ctx.lower_event_specs(specs);
                    (wait, std::mem::take(&mut ctx.pre_fns))
                };
                let wait = wait.map_err(|error| {
                    self.coverage_error(
                        *timing,
                        format!("sampling event of covergroup `{name}`: {error}"),
                    )
                })?;
                self.func = None;
                let body = vec![
                    IrStmt::WaitEvents {
                        specs: wait,
                        refresh: false,
                    },
                    IrStmt::Coverage(Box::new(IrCoverageStmt::SampleRegistered {
                        group: lowering.ir,
                        function: lowering.sample,
                        depth: IrDepth::PROC,
                        strobe: lowering.strobe,
                    })),
                ];
                self.model.processes.push(IrProcess::new_with_origin(
                    process_name,
                    format!("{path}.{name}.sample"),
                    IrShape::Loop,
                    pre_fns,
                    body,
                    crate::sim::semantic::Origin::Synthetic {
                        reason: format!(
                            "sampling event of covergroup {}",
                            self.node(group).full_name
                        ),
                    },
                ));
                Ok(())
            }
        }
    }

    /// The named block statement a block event names. The reference names
    /// the block's label scope: a named block that the statement with the
    /// same name lists among its children. The scope holds only
    /// declarations, which include the scopes of named blocks nested in it.
    fn named_block_statement(&self, target: NodeId) -> Option<NodeId> {
        let is_begin = |node: NodeId| matches!(self.kind(node), NodeKind::Stmt(StmtKind::Begin));
        let name = &self.node(target).name;
        if !is_begin(target) || name.is_empty() {
            return None;
        }
        self.db.node_ids().find(|statement| {
            *statement != target
                && is_begin(*statement)
                && self.node(*statement).name == *name
                && self.node(*statement).children.contains(&target)
        })
    }

    /// Statements sampling every registered instance of the groups whose
    /// block event names subroutine `target` at its start (`begin`) or end.
    pub(in super::super) fn coverage_block_samples(
        &mut self,
        target: NodeId,
        begin: bool,
    ) -> Result<Vec<IrStmt>, String> {
        let Some(events) = self.coverage_block_events.get(&target).cloned() else {
            return Ok(Vec::new());
        };
        self.coverage_block_events_applied.insert(target);
        let depth = parse_depth(&self.depth_arg);
        let mut statements = Vec::new();
        for (group, event_begin) in events {
            if event_begin != begin {
                continue;
            }
            let Some(lowering) = self.covergroups.get(&group) else {
                continue;
            };
            statements.push(IrStmt::Coverage(Box::new(
                IrCoverageStmt::SampleRegistered {
                    group: lowering.ir,
                    function: lowering.sample,
                    depth,
                    strobe: false,
                },
            )));
        }
        Ok(statements)
    }

    /// Every block event must have been attached to its subroutine body.
    pub(in super::super) fn check_coverage_block_events(&self) -> Result<(), String> {
        let mut targets = self
            .coverage_block_events
            .keys()
            .filter(|target| !self.coverage_block_events_applied.contains(target))
            .copied()
            .collect::<Vec<_>>();
        targets.sort_by_key(|target| target.index());
        match targets.first() {
            None => Ok(()),
            Some(target) => Err(self.coverage_error(
                *target,
                format!(
                    "the covergroup block event on `{}` is not supported for this subroutine (an imported or extern subroutine without its own body)",
                    self.node(*target).name
                ),
            )),
        }
    }

    /// The covergroup, coverpoint and method a built-in coverage method
    /// declaration belongs to.
    fn coverage_method(&self, callee: NodeId) -> Option<(NodeId, Option<NodeId>, CoverageMethod)> {
        let method = match self.node(callee).name.as_str() {
            "sample" => CoverageMethod::Sample,
            "get_coverage" => CoverageMethod::GetCoverage,
            "get_inst_coverage" => CoverageMethod::GetInstCoverage,
            "start" => CoverageMethod::Start,
            "stop" => CoverageMethod::Stop,
            "set_inst_name" => CoverageMethod::SetInstName,
            _ => return None,
        };
        let parent = self.node(callee).parent?;
        if self.db.coverpoint(parent).is_some() {
            let group = self.db.coverpoint(parent)?.group;
            return Some((group, Some(parent), method));
        }
        if let Some(cross) = self.db.coverage_cross(parent) {
            return Some((cross.group, Some(parent), method));
        }
        let group = self.node(parent).parent?;
        self.db.covergroup(group)?;
        Some((group, None, method))
    }

    fn coverage_call(&self, node: NodeId) -> Option<(NodeId, Option<NodeId>, CoverageMethod)> {
        match self.kind(node) {
            NodeKind::MethodCall {
                callee: Some(callee),
                ..
            }
            | NodeKind::FuncCall {
                callee: Some(callee),
                ..
            } => self.coverage_method(*callee),
            _ => None,
        }
    }

    /// The covergroup object a coverage method call's receiver names.
    fn coverage_receiver(
        &mut self,
        path: &str,
        node: NodeId,
        point: Option<NodeId>,
    ) -> Result<IrChandleExpr, String> {
        let NodeKind::MethodCall {
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        else {
            // An unqualified call inside the group's own scope has no object.
            return Err(
                self.coverage_error(node, "coverage method call has no covergroup instance")
            );
        };
        // `c.cp.method()` selects the coverpoint of the instance `c` names.
        let handle_node = match (point, self.kind(*receiver)) {
            (Some(_), NodeKind::Expr(ExprKind::HierPath { .. })) => self
                .node(*receiver)
                .children
                .first()
                .copied()
                .ok_or_else(|| self.coverage_error(node, "coverpoint selection has no instance"))?,
            _ => *receiver,
        };
        let handle = self.lower_chandle(path, handle_node)?;
        Ok(IrChandleExpr::Required {
            handle: Box::new(handle),
            site: self.source_site("covergroup method call", path, node),
        })
    }

    fn coverage_record(
        &mut self,
        group: NodeId,
        object: IrChandleExpr,
    ) -> Result<IrChandleExpr, String> {
        let lowering = self
            .covergroups
            .get(&group)
            .ok_or_else(|| self.coverage_error(group, "covergroup is never constructed"))?;
        let (class, field) = (lowering.class, lowering.record_field);
        Ok(IrChandleExpr::LocalRead(self.native_access_symbol(
            object,
            IrNativeAccessKind::ClassField { class, field },
        )))
    }

    fn coverage_point_index(&self, group: NodeId, point: Option<NodeId>) -> Option<u32> {
        point.and_then(|point| self.covergroups.get(&group)?.points.get(&point).copied())
    }

    /// A coverage query in expression position: `get_coverage()`,
    /// `get_inst_coverage()` (19.8) and `$get_coverage()` (19.9).
    pub(in super::super) fn lower_coverage_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let real = |query: IrCoverageQuery| {
            IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::Coverage(Box::new(query)))),
                REAL_EXPR_WIDTH,
                true,
                None,
            )
        };
        if let NodeKind::SysCall { name } = self.kind(node) {
            if name == "$get_coverage" {
                if !self.node(node).children.is_empty() {
                    return Err(self.coverage_error(node, "$get_coverage takes no arguments"));
                }
                return Ok(Some(real(IrCoverageQuery::Total)));
            }
            return Ok(None);
        }
        if let Some(read) = self.coverage_ref_read(path, node)? {
            return Ok(Some(read));
        }
        if let Some(read) = self.lower_coverage_option_read(path, node)? {
            return Ok(Some(read));
        }
        let Some((group, point, method)) = self.coverage_call(node) else {
            return Ok(None);
        };
        if !matches!(
            method,
            CoverageMethod::GetCoverage | CoverageMethod::GetInstCoverage
        ) {
            return Err(self.coverage_error(
                node,
                format!("covergroup method `{}` has no value", self.node(node).name),
            ));
        }
        let point_index = match point {
            Some(point) => Some(match self.coverage_point_index(group, Some(point)) {
                Some(index) => index,
                None => self
                    .coverage_item_position(node, group, Some(point))?
                    .unwrap_or_default(),
            }),
            None => None,
        };
        let query = if method == CoverageMethod::GetCoverage {
            IrCoverageQuery::Type {
                coverage_type: self.coverage_type_index(group),
                point: point_index,
            }
        } else {
            let object = self.coverage_receiver(path, node, point)?;
            let record = self.coverage_record(group, object)?;
            IrCoverageQuery::Instance {
                record,
                point: point_index,
            }
        };
        // Defaulted `covered_bins`/`total_bins` actuals are the declaration's
        // shared default expressions; only explicit ones belong to the call.
        let explicit = self
            .call_argument_nodes(node)
            .into_iter()
            .filter(|arg| self.node(*arg).parent == Some(node))
            .collect::<Vec<_>>();
        if explicit.is_empty() {
            return Ok(Some(real(query)));
        }
        let [covered, total] = explicit.as_slice() else {
            return Err(self.coverage_error(
                node,
                format!(
                    "`{}` takes either no arguments or both covered and total bin arguments (IEEE 1800-2009 19.8)",
                    self.node(node).name
                ),
            ));
        };
        let mut targets = Vec::with_capacity(2);
        for actual in [*covered, *total] {
            let target = self
                .lower_plusarg_target(path, actual, "coverage bin count")
                .map_err(|error| self.coverage_error(actual, error))?;
            if !matches!(
                target,
                IrPlusArgTarget::Packed {
                    width: 32,
                    signed: true,
                    ..
                }
            ) {
                return Err(self.coverage_error(
                    actual,
                    "covered and total bin arguments must be int variables (IEEE 1800-2009 19.8)",
                ));
            }
            targets.push(target);
        }
        let total = targets.pop().ok_or("coverage bin count target vanished")?;
        let covered = targets.pop().ok_or("coverage bin count target vanished")?;
        Ok(Some(IrExpr::new(
            IrExprKind::SysFunc(Box::new(IrSysFunc::CoverageCounts {
                query: Box::new(query),
                covered,
                total,
            })),
            REAL_EXPR_WIDTH,
            true,
            None,
        )))
    }

    /// The actual a `ref` formal names while a covergroup body or waiter
    /// lowers, for an expression that is exactly a read of that formal.
    pub(in super::super) fn coverage_ref_actual(&self, expression: NodeId) -> Option<NodeId> {
        match self.kind(expression) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.coverage_ref_actuals.get(target).copied(),
            _ => None,
        }
    }

    /// A read of a `ref` formal while a covergroup body lowers: the variable
    /// the current binding variant names.
    fn coverage_ref_read(&mut self, path: &str, node: NodeId) -> Result<Option<IrExpr>, String> {
        if self.coverage_ref_actuals.is_empty() {
            return Ok(None);
        }
        let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        else {
            return Ok(None);
        };
        let Some(actual) = self.coverage_ref_actuals.get(target).copied() else {
            return Ok(None);
        };
        self.lower_expr(path, actual).map(Some)
    }

    /// A coverage method call in statement position: `sample()`, `start()`,
    /// `stop()` and `set_inst_name()`.
    pub(in super::super) fn lower_coverage_statement(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let Some((group, point, method)) = self.coverage_call(node) else {
            return Ok(None);
        };
        let name = self.covergroup_name(group);
        match method {
            CoverageMethod::GetCoverage | CoverageMethod::GetInstCoverage => {
                // A discarded query has no effect beyond its arguments.
                self.lower_coverage_expr(path, node)?;
                Ok(Some(IrStmt::Nop))
            }
            CoverageMethod::SetInstName => {
                // 19.8: the instance name (option.name) becomes the argument.
                let object = self.coverage_receiver(path, node, None)?;
                let record = self.coverage_record(group, object)?;
                let args = self.call_argument_nodes(node);
                let [name_arg] = args.as_slice() else {
                    return Err(self.coverage_error(
                        node,
                        "`set_inst_name` takes one string argument (IEEE 1800-2009 19.8)",
                    ));
                };
                let value = self.lower_string(path, *name_arg)?;
                Ok(Some(IrStmt::Coverage(Box::new(IrCoverageStmt::Text {
                    record,
                    point: None,
                    which: IrCoverageText::Name,
                    value,
                }))))
            }
            CoverageMethod::Start | CoverageMethod::Stop => {
                let object = self.coverage_receiver(path, node, point)?;
                let record = self.coverage_record(group, object)?;
                let point = match point {
                    Some(point) => Some(self.coverage_point_index(group, Some(point)).ok_or_else(
                        || self.coverage_error(node, "coverpoint is not in its covergroup"),
                    )?),
                    None => None,
                };
                Ok(Some(IrStmt::Coverage(Box::new(IrCoverageStmt::Control {
                    record,
                    point,
                    start: method == CoverageMethod::Start,
                }))))
            }
            CoverageMethod::Sample => {
                let lowering = self.covergroups.get(&group).cloned().ok_or_else(|| {
                    self.coverage_error(node, format!("covergroup `{name}` is never constructed"))
                })?;
                let receiver = self.coverage_receiver(path, node, None)?;
                let formals = self.model.funcs[lowering.sample].formals.clone();
                let args = self.call_argument_nodes(node);
                if args.len() != formals.len() {
                    return Err(self.coverage_error(
                        node,
                        format!(
                            "`sample` of covergroup `{name}` takes {} argument(s), but {} were given (IEEE 1800-2009 19.8.1)",
                            formals.len(),
                            args.len()
                        ),
                    ));
                }
                let mut lowered = Vec::with_capacity(args.len());
                for (arg, formal) in args.iter().zip(&formals) {
                    let value = self.lower_expr(path, *arg)?;
                    let value = ir_to_storage(value, formal.width, formal.signed, formal.two_state)
                        .map_err(|error| self.coverage_error(*arg, error))?;
                    lowered.push(IrCallArg::Val(value));
                }
                let mut call = IrCall::new(
                    lowering.sample,
                    lowered,
                    parse_depth(&self.depth_arg),
                    Vec::new(),
                    Vec::new(),
                );
                call.receiver = Some(receiver);
                Ok(Some(IrStmt::Call(Box::new(call))))
            }
        }
    }

    /// `new(...)` of a covergroup (19.3, 19.4): allocate the instance object,
    /// bind its arguments, create the runtime record, apply the options
    /// and register it with its sampling event.
    pub(in super::super) fn lower_new_covergroup(
        &mut self,
        path: &str,
        node: NodeId,
        group_type: Option<TypeId>,
        args: &[NodeId],
    ) -> Result<IrChandleExpr, String> {
        let group = self.covergroup_of_type(group_type).ok_or_else(|| {
            self.coverage_error(node, "covergroup `new` names no covergroup type")
        })?;
        let lowering = self
            .covergroups
            .get(&group)
            .cloned()
            .ok_or_else(|| self.coverage_error(node, "covergroup has no lowered layout"))?;
        let info = self
            .db
            .covergroup(group)
            .cloned()
            .ok_or("covergroup vanished")?;
        let name = self.covergroup_name(group);
        // 19.4: an embedded group is constructed only by its class's `new`.
        let owner_receiver = if let Some(owner_field) = lowering.owner_field {
            let owner_class = self.node(group).parent;
            let in_constructor = self
                .func
                .as_ref()
                .and_then(|function| function.def_node)
                .is_some_and(|method| {
                    matches!(
                        self.kind(method),
                        NodeKind::FuncTask {
                            is_constructor: true,
                            ..
                        }
                    ) && self.node(method).parent == owner_class
                });
            let receiver = self
                .class_init_receiver
                .clone()
                .or_else(|| self.method_receiver());
            match (in_constructor, receiver) {
                (true, Some(receiver)) => Some((owner_field, receiver)),
                _ => {
                    return Err(self.coverage_error(
                        node,
                        format!(
                            "embedded covergroup `{name}` may only be constructed in the `new` method of its class (IEEE 1800-2009 19.4)"
                        ),
                    ))
                }
            }
        } else {
            None
        };
        if args.len() != info.formals.len() {
            return Err(self.coverage_error(
                node,
                format!(
                    "covergroup `{name}` takes {} argument(s), but {} were given",
                    info.formals.len(),
                    args.len()
                ),
            ));
        }
        let object_name = format!("_llg_obj_{}", node.index());
        let object = IrChandleExpr::LocalRead(object_name.clone());
        let access = |cg: &mut Codegen<'_>, field: usize| {
            cg.native_access_symbol(
                object.clone(),
                IrNativeAccessKind::ClassField {
                    class: lowering.class,
                    field,
                },
            )
        };
        let mut body = Vec::new();
        if let Some((field, receiver)) = owner_receiver {
            let target = access(self, field);
            body.push(IrStmt::Object(Box::new(
                crate::sim::ir::IrObjectStmt::ChandleAssignLocal(target, receiver),
            )));
        }
        if let Some(field) = lowering.bind_field {
            let variant = lowering.variant_of_new.get(&node).copied().unwrap_or(0);
            let target = access(self, field);
            body.push(IrStmt::Assign {
                lhs: IrLhs::WholeRef {
                    addr: format!("&{target}"),
                    width: 32,
                    signed: false,
                    two_state: true,
                    shortreal: false,
                },
                rhs: lhs_integer_expr(i128::from(variant)),
                nba: false,
            });
        }
        // Input arguments are evaluated when `new` executes (19.3).
        for (formal, actual) in info.formals.iter().zip(args) {
            let Some((_, field)) = self.class_fields.get(formal).copied() else {
                continue;
            };
            let field_ty = self.model.classes[lowering.class].fields[field].ty;
            let target = access(self, field);
            match field_ty {
                IrClassFieldType::Chandle => {
                    let value = self.lower_chandle(path, *actual)?;
                    body.push(IrStmt::Object(Box::new(
                        crate::sim::ir::IrObjectStmt::ChandleAssignLocal(target, value),
                    )));
                }
                IrClassFieldType::Real { shortreal } => {
                    let value = self.lower_expr(path, *actual)?;
                    let value = if value.is_real() {
                        value
                    } else {
                        IrExpr::new(
                            IrExprKind::CastToReal {
                                a: Box::new(value),
                                shortreal,
                            },
                            REAL_EXPR_WIDTH,
                            true,
                            None,
                        )
                    };
                    body.push(IrStmt::Assign {
                        lhs: IrLhs::WholeRef {
                            addr: format!("&{target}"),
                            width: 0,
                            signed: false,
                            two_state: false,
                            shortreal,
                        },
                        rhs: value,
                        nba: false,
                    });
                }
                IrClassFieldType::Packed {
                    width,
                    signed,
                    two_state,
                } => {
                    let value = self.lower_expr(path, *actual)?;
                    body.push(IrStmt::Assign {
                        lhs: IrLhs::WholeRef {
                            addr: format!("&{target}"),
                            width,
                            signed,
                            two_state,
                            shortreal: false,
                        },
                        rhs: ir_to_storage(value, width, signed, two_state)?,
                        nba: false,
                    });
                }
                IrClassFieldType::String => {
                    let value = self.lower_string(path, *actual)?;
                    body.push(IrStmt::Object(Box::new(
                        crate::sim::ir::IrObjectStmt::StringAssignLocal(target, value),
                    )));
                }
            }
        }
        let record_target = access(self, lowering.record_field);
        body.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Create {
            group: lowering.ir,
            target: record_target.clone(),
        })));
        let record = IrChandleExpr::LocalRead(record_target);
        // Option setters run at instantiation (19.7) in the declaration's
        // scope: arguments are the new object's fields, owner properties
        // the constructing object's.
        let saved_scope = self
            .coverage_scope
            .replace((lowering.class, object.clone()));
        let saved_actuals = std::mem::take(&mut self.coverage_ref_actuals);
        if let Some(variant) = lowering
            .variants
            .get(lowering.variant_of_new.get(&node).copied().unwrap_or(0) as usize)
        {
            self.coverage_ref_actuals = variant.clone();
        }
        let options = (|| -> Result<Vec<IrStmt>, String> {
            let mut statements = Vec::new();
            let scopes = std::iter::once((None, info.options.clone()))
                .chain(info.coverpoints.iter().enumerate().map(|(index, point)| {
                    (
                        Some(index as u32),
                        self.db
                            .coverpoint(*point)
                            .map(|metadata| metadata.options.clone())
                            .unwrap_or_default(),
                    )
                }))
                .chain(info.crosses.iter().enumerate().map(|(index, cross)| {
                    (
                        Some((info.coverpoints.len() + index) as u32),
                        self.db
                            .coverage_cross(*cross)
                            .map(|metadata| metadata.options.clone())
                            .unwrap_or_default(),
                    )
                }));
            for (point, setters) in scopes.collect::<Vec<_>>() {
                for setter in setters {
                    let Some((scope, member, value)) = self.coverage_option_parts(setter) else {
                        continue;
                    };
                    if scope != "option" {
                        continue;
                    }
                    if matches!(member.as_str(), "name" | "comment") {
                        let which = if member == "name" {
                            IrCoverageText::Name
                        } else {
                            IrCoverageText::Comment
                        };
                        let value = self
                            .lower_string(path, value)
                            .map_err(|error| self.coverage_error(value, error))?;
                        statements.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Text {
                            record: record.clone(),
                            point,
                            which,
                            value,
                        })));
                        continue;
                    }
                    let Some(option) = coverage_instance_option(&member) else {
                        continue;
                    };
                    let lowered = self
                        .lower_expr(path, value)
                        .map_err(|error| self.coverage_error(value, error))?;
                    if lowered.is_real() {
                        return Err(self.coverage_error(
                            value,
                            format!("option.{member} needs an integral value"),
                        ));
                    }
                    statements.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Option {
                        record: record.clone(),
                        point,
                        option,
                        value: lowered,
                        site: self.source_site(&format!("option.{member}"), path, setter),
                    })));
                }
            }
            // Bins values that read arguments, in the declaration's scope
            // like the options (19.5).
            for (slot, node) in lowering.build.slots.iter().copied().enumerate() {
                let value = self
                    .lower_expr(path, node)
                    .map_err(|error| self.coverage_error(node, error))?;
                if value.is_real() || value.width == 0 || value.width > 64 {
                    return Err(self.coverage_error(
                        node,
                        "bins values that read covergroup arguments must be integral values of at most 64 bits",
                    ));
                }
                statements.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Slot {
                    record: record.clone(),
                    slot: slot as u32,
                    value,
                })));
            }
            Ok(statements)
        })();
        self.coverage_scope = saved_scope;
        self.coverage_ref_actuals = saved_actuals;
        body.extend(options?);
        body.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Ready { record })));
        if lowering.registered {
            body.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Register {
                group: lowering.ir,
                object: object.clone(),
            })));
        }
        if let Some(waiter) = lowering.waiter.clone() {
            // The waiter belongs to the instance, not to the constructing
            // process (decision S44-D5).
            body.push(IrStmt::Coverage(Box::new(IrCoverageStmt::Watch {
                object: object.clone(),
                waiter,
                label: format!("{}.{name}.sample", self.source_path(path)),
            })));
        }
        let index = self.model.class_allocations.len();
        self.model
            .class_allocations
            .push(crate::sim::ir::IrClassAllocation {
                class: lowering.class,
                local: object_name,
                body,
                function: self.cur_fn_ir,
            });
        Ok(IrChandleExpr::Construct(index))
    }

    /// While a covergroup body or option lowers, its arguments are fields of
    /// the covergroup object rather than of the method's receiver.
    pub(in super::super) fn coverage_receiver_for(&self, field: NodeId) -> Option<IrChandleExpr> {
        let (class, object) = self.coverage_scope.as_ref()?;
        let (field_class, _) = self.class_fields.get(&field)?;
        (field_class == class).then(|| object.clone())
    }
}
