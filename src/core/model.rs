//! model — owned design model over the elaborated UHDM, shared by the LSP
//! (symbols, hover, completion) and the simulator (codegen input).
//!
//! [`DesignModel`] is fully owned: no `VpiHandle`, no lifetimes, `Send + Sync`.
//! It is extracted from the elaborated UHDM while the surelog session is alive
//! (see [`DesignModel::build`]), after which all raw handles are gone.
//!
//! Extraction walks the elaborated UHDM once into [`core::db::Db`]; the node
//! arena is then shaped into the model structures by
//! [`DesignModel::from_db`].  The typespec helpers below (`typespec_info` and
//! friends) are retained as a handle-based public API for callers that still
//! hold VPI handles (tests, tooling); they read values through
//! [`vpi::read_value`] with no raw FFI access.

use std::collections::HashSet;

use crate::core::db::{self, NodeId};
use crate::core::elab::Val;
use crate::ffi::vpi::{self, OwnedHandle, ValueData, VpiHandle};

/// Port direction, from `vpiDirection` on the port object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
    Inout,
    None,
}

/// Kind of a named design object, used by LSP symbol classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymKind {
    Module,
    Interface,
    Package,
    Instance,
    Port,
    Net,
    Var,
    Param,
    GenScope,
    EnumConst,
    Typedef,
    Function,
    Task,
    Class,
    Program,
}

/// A resolved type: base kind, packed width and signedness.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeInfo {
    /// Base type kind: `"logic"`, `"bit"`, `"int"`, `"integer"`, `"longint"`,
    /// `"byte"`, `"shortint"`, `"time"`, `"real"`, `"shortreal"`, `"enum"`, `"struct"`,
    /// `"union"`, `"array"`, `"string"`, `"class"`, `"other"`.
    pub kind: String,
    /// Bit width; `None` when unknown or unsized (e.g. `string`).
    pub width: Option<u32>,
    pub signed: bool,
    /// Name of the underlying enum/struct/union/typedef, if any.
    pub type_name: Option<String>,
}

impl Default for TypeInfo {
    fn default() -> Self {
        TypeInfo {
            kind: "other".to_string(),
            width: None,
            signed: false,
            type_name: None,
        }
    }
}

impl TypeInfo {
    /// Short deterministic rendering, e.g. `"logic [7:0]"`, `"int"`,
    /// `"enum state_t"`, `"struct pair_t"`.  A `signed` flag is prefixed only
    /// for non-integer kinds (integer kinds are signed by language rules).
    pub fn render(&self) -> String {
        let prefix = if self.signed
            && !matches!(
                self.kind.as_str(),
                "int" | "integer" | "longint" | "byte" | "shortint" | "time" | "real" | "shortreal"
            ) {
            "signed "
        } else {
            ""
        };
        match self.kind.as_str() {
            "logic" | "bit" => match self.width {
                Some(w) if w > 1 => format!("{}{} [{}:0]", prefix, self.kind, w - 1),
                _ => format!("{}{}", prefix, self.kind),
            },
            "enum" | "struct" | "union" => match &self.type_name {
                Some(n) => format!("{} {}", self.kind, n),
                None => self.kind.clone(),
            },
            "class" => match &self.type_name {
                Some(n) => format!("class {n}"),
                None => "class".to_string(),
            },
            "array" | "real_array" | "shortreal_array" => "array".to_string(),
            other => other.to_string(),
        }
    }
}

/// One module port.
#[derive(Debug, Clone, PartialEq)]
pub struct PortModel {
    pub name: String,
    pub direction: Direction,
    pub ty: TypeInfo,
}

/// One signal: a net, a variable, or an array.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalModel {
    pub name: String,
    /// A net-kind name (`"wire"`, `"wand"`, …), `"var"` or `"array"`.
    pub kind: String,
    pub ty: TypeInfo,
}

/// One parameter (or localparam).  `value` is `None` when the parameter
/// could not be resolved (e.g. cyclic dependencies).
#[derive(Debug, Clone, PartialEq)]
pub struct ParamModel {
    pub name: String,
    pub value: Option<Val>,
    pub ty: TypeInfo,
    pub local: bool,
}

/// One formal argument of a function/task, in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncArgDef {
    pub name: String,
    pub direction: Direction,
    pub ty: TypeInfo,
    /// `true` when the formal carries a default value (`input int a = 7`).
    pub has_default: bool,
}

/// A function or task definition, captured per instance (Surelog elaborates
/// one clone per instantiated scope, with all refs re-bound).
#[derive(Debug, Clone, PartialEq)]
pub struct FuncDef {
    pub name: String,
    /// `true` for a task, `false` for a function.
    pub is_task: bool,
    pub automatic: bool,
    /// Declaration site of the function/task (the definition file — the same
    /// file for every per-instance clone).
    pub file: Option<String>,
    /// Declaration line (1-based).
    pub line: u32,
    /// Declaration column (1-based).
    pub col: u32,
    /// Return type; `None` for void functions and tasks.
    pub ret: Option<TypeInfo>,
    pub args: Vec<FuncArgDef>,
    /// Full name of the instance the clone belongs to.
    pub scope: String,
}

/// One elaborated generate block instance, e.g. `gen_blk[0]`.
#[derive(Debug, Clone, PartialEq)]
pub struct GenScopeModel {
    pub name: String,
    /// `vpiFullName`, when Surelog provides it.  This includes the concrete
    /// generate iteration (for example `work@top.g[0]`) and is therefore a
    /// better identity for consumers than the display name alone.
    pub full_name: String,
    /// Gen-scope parameters (genvars) with their concrete values.
    pub params: Vec<ParamModel>,
    /// Module/interface instances elaborated inside this generate scope.
    /// Nested generate scopes are intentionally not flattened here; the
    /// direct children are enough for hierarchy consumers to preserve the
    /// generated boundary without inventing source-level structure.
    pub children: Vec<InstanceModel>,
}

/// One module instance.  For the top instance `name` carries the module
/// *definition* name (a Surelog quirk: the top instance's `vpiName` is the
/// def name, and it has no `vpiFullName`).
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceModel {
    pub name: String,
    pub def_name: String,
    /// `vpiFullName`; falls back to `name` for the top instance.
    pub full_name: String,
    /// Instantiation site file; the top instance falls back to the parent's
    /// file (its own `vpiFile` is the def file).
    pub file: Option<String>,
    /// Instantiation site line (1-based).
    pub line: u32,
    /// Instantiation site column (1-based).
    pub col: u32,
    pub ports: Vec<PortModel>,
    pub signals: Vec<SignalModel>,
    pub params: Vec<ParamModel>,
    pub gen_scopes: Vec<GenScopeModel>,
    /// Function/task definitions of this instance (per-instance clones).
    pub funcs: Vec<FuncDef>,
    pub children: Vec<InstanceModel>,
}

impl InstanceModel {
    /// The function/task definition named `name` on this instance (first
    /// match in declaration order).
    ///
    /// Per-instance clones mean the same name can legitimately appear more
    /// than once (e.g. an interface view and the actual instance it binds to);
    /// every [`FuncDef`] carries its own `scope` (instance full name) and
    /// declaration position, so callers that need a specific clone should
    /// filter [`Self::funcs`] by scope/position instead of relying on the
    /// first match.
    pub fn func(&self, name: &str) -> Option<&FuncDef> {
        self.funcs.iter().find(|f| f.name == name)
    }
}

/// A module *definition* (flat object under `uhdmallModules`), carrying the
/// declaration positions.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDef {
    pub name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// A package definition.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageDef {
    pub name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    /// Package parameters (`parameter`/`localparam` declarations), in
    /// declaration order.  `value` is `None` when the parameter could not be
    /// resolved (e.g. cyclic dependencies).
    pub params: Vec<ParamModel>,
    /// Enum constants declared in the package's enum typedefs, in declaration
    /// order.  `value` is `None` when the constant could not be read.
    pub enum_consts: Vec<EnumConstDef>,
}

/// One enum constant declared in a package's enum typedef, with its position.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumConstDef {
    pub name: String,
    /// Resolved enumerator value; `None` when the constant has no readable
    /// value (e.g. an expression Surelog did not fold).
    pub value: Option<Val>,
    pub file: Option<String>,
    /// Declaration line (1-based).
    pub line: u32,
    /// Declaration column (1-based).
    pub col: u32,
}

/// One class data member (field), in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassFieldDef {
    pub name: String,
    pub ty: TypeInfo,
    /// Declaration line (1-based).
    pub line: u32,
    /// Declaration column (1-based).
    pub col: u32,
}

/// A class definition (per-file, not per-instance).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: String,
    pub file: Option<String>,
    /// Declaration line of the `class` keyword (1-based).
    pub line: u32,
    /// Declaration column of the `class` keyword (1-based).
    pub col: u32,
    /// Class methods in declaration order (`new`, functions, tasks).  The
    /// constructor is a function named `new`; its `ret` is `None` (the
    /// implicit class-handle return is not spelled in source).
    pub methods: Vec<FuncDef>,
    /// Class data members in declaration order.
    pub fields: Vec<ClassFieldDef>,
}

/// Fully owned snapshot of the elaborated design.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignModel {
    pub design_name: String,
    pub top_instances: Vec<InstanceModel>,
    pub modules: Vec<ModuleDef>,
    pub packages: Vec<PackageDef>,
    pub classes: Vec<ClassDef>,
}

impl DesignModel {
    /// Build the model from an elaborated UHDM design handle.  The owning
    /// surelog session must stay alive for the duration of the call.
    ///
    /// Parameter values come from [`crate::core::elab::Resolver`];
    /// unresolvable parameters (e.g. cyclic ones) are emitted with
    /// `value: None` rather than failing the whole build.
    pub fn build(design: VpiHandle) -> Result<DesignModel, db::DbError> {
        Ok(Self::from_db(&db::Db::build(design)?))
    }

    /// Shape an owned [`db::Db`] into the model structures.
    ///
    /// Fully owned: the returned model shares nothing with the database or
    /// the VPI session it was built from.
    pub fn from_db(db: &db::Db) -> DesignModel {
        let top_instances = db.tops.iter().map(|id| instance_from_db(db, *id)).collect();
        let modules = db
            .flat_modules
            .iter()
            .map(|id| module_def_from_db(db, *id))
            .collect();
        let packages = db
            .packages
            .iter()
            .map(|id| package_from_db(db, *id))
            .collect();
        let classes = db.classes.iter().map(|id| class_from_db(db, *id)).collect();
        DesignModel {
            design_name: db.design_name.clone(),
            top_instances,
            modules,
            packages,
            classes,
        }
    }

    /// Find an instance by exact full name (e.g. `"top.u0"`).
    ///
    /// Surelog prefixes instance full names with the design unit
    /// (`"work@top.u0"`); this method also accepts the name with that
    /// `<library>@` prefix stripped, so both spellings match.
    pub fn instance(&self, full_name: &str) -> Option<&InstanceModel> {
        fn strip_lib(s: &str) -> &str {
            s.split_once('@').map(|(_, rest)| rest).unwrap_or(s)
        }
        fn matches(stored: &str, want: &str) -> bool {
            stored == want || strip_lib(stored) == want
        }
        let mut stack: Vec<&InstanceModel> = self.top_instances.iter().collect();
        while let Some(inst) = stack.pop() {
            if matches(&inst.full_name, full_name) {
                return Some(inst);
            }
            stack.extend(inst.children.iter());
        }
        None
    }

    /// Module defs whose declaration file matches `file` exactly.
    pub fn modules_in(&self, file: &str) -> Vec<&ModuleDef> {
        self.modules
            .iter()
            .filter(|m| m.file.as_deref() == Some(file))
            .collect()
    }
}

// ── Db → model shaping ────────────────────────────────────────────────────────

fn instance_from_db(db: &db::Db, id: NodeId) -> InstanceModel {
    let node = db.node(id);
    let def_name = match &node.kind {
        db::NodeKind::ModuleInst { def_name, .. } => def_name.clone(),
        _ => String::new(),
    };
    let name = node.name.clone();
    let full_name = if node.full_name.is_empty() {
        name.clone()
    } else {
        node.full_name.clone()
    };
    let file = node.file.clone();
    let line = node.line;
    let col = node.col;

    let ports: Vec<PortModel> = node
        .children
        .iter()
        .filter_map(|c| match db.node_kind(*c) {
            db::NodeKind::Port { .. } => Some(port_from_db(db, *c)),
            _ => None,
        })
        .collect();
    let port_names: HashSet<&str> = ports.iter().map(|port| port.name.as_str()).collect();
    let signals = node
        .children
        .iter()
        .filter_map(|c| {
            let (kind, ty) = match db.node_kind(*c) {
                db::NodeKind::Net { ty, net_type } => (net_kind(*net_type), ty),
                db::NodeKind::Var { ty } => ("var", ty),
                db::NodeKind::Array { ty } => ("array", ty),
                _ => return None,
            };
            let name = db.node(*c).name.as_str();
            // UHDM exposes the backing net/variable for many ports as a
            // sibling of the port object.  Signals are internal-only in the
            // explorer, so do not duplicate formal ports here.
            if port_names.contains(name) {
                return None;
            }
            Some(SignalModel {
                name: name.to_owned(),
                kind: kind.to_owned(),
                ty: ty.clone(),
            })
        })
        .collect();
    let params = node
        .children
        .iter()
        .filter_map(|c| match db.node_kind(*c) {
            db::NodeKind::Param { ty, value, local } => Some(ParamModel {
                name: db.node(*c).name.clone(),
                value: value.clone(),
                ty: ty.clone(),
                local: *local,
            }),
            _ => None,
        })
        .collect();
    let gen_scopes = node
        .children
        .iter()
        .flat_map(|c| match db.node_kind(*c) {
            db::NodeKind::GenScopeArray => gen_scopes_from_db(db, *c),
            _ => Vec::new(),
        })
        .collect();
    // Per-instance function/task clones.  Each clone's declaration site is the
    // definition file/position; the scope is this instance's full name.
    let funcs = node
        .children
        .iter()
        .filter_map(|c| match db.node_kind(*c) {
            db::NodeKind::FuncTask {
                is_task,
                automatic,
                ret,
            } => Some(FuncDef {
                name: db.node(*c).name.clone(),
                is_task: *is_task,
                automatic: *automatic,
                file: db.node(*c).file.clone(),
                line: db.node(*c).line,
                col: db.node(*c).col,
                ret: ret.clone(),
                args: func_args_from_db(db, *c),
                scope: full_name.clone(),
            }),
            _ => None,
        })
        .collect();
    let children = node
        .children
        .iter()
        .filter(|&c| matches!(db.node_kind(*c), db::NodeKind::ModuleInst { .. }))
        .map(|c| instance_from_db(db, *c))
        .collect();

    InstanceModel {
        name,
        def_name,
        full_name,
        file,
        line,
        col,
        ports,
        signals,
        params,
        gen_scopes,
        funcs,
        children,
    }
}

/// Formal arguments of a function/task node, in declaration order (the
/// `FuncArg` children of the `FuncTask` node).
fn func_args_from_db(db: &db::Db, ft_id: NodeId) -> Vec<FuncArgDef> {
    db.node(ft_id)
        .children
        .iter()
        .filter_map(|c| match &db.node_kind(*c) {
            db::NodeKind::FuncArg {
                direction,
                ty,
                default,
            } => Some(FuncArgDef {
                name: db.node(*c).name.clone(),
                direction: *direction,
                ty: ty.clone(),
                has_default: default.is_some(),
            }),
            _ => None,
        })
        .collect()
}

fn net_kind(net_type: i32) -> &'static str {
    match net_type {
        vpi::vpiWire => "wire",
        vpi::vpiWand => "wand",
        vpi::vpiWor => "wor",
        vpi::vpiTri => "tri",
        vpi::vpiTri0 => "tri0",
        vpi::vpiTri1 => "tri1",
        vpi::vpiTriReg => "trireg",
        vpi::vpiTriAnd => "triand",
        vpi::vpiTriOr => "trior",
        vpi::vpiSupply0 => "supply0",
        vpi::vpiSupply1 => "supply1",
        vpi::vpiUwire => "uwire",
        _ => "net",
    }
}

fn module_def_from_db(db: &db::Db, id: NodeId) -> ModuleDef {
    let node = db.node(id);
    let name = match &node.kind {
        db::NodeKind::ModuleInst { def_name, .. } => def_name.clone(),
        _ => String::new(),
    };
    ModuleDef {
        name,
        file: node.file.clone(),
        line: node.line,
        col: node.col,
        end_line: node.end_line,
        end_col: node.end_col,
    }
}

fn package_from_db(db: &db::Db, id: NodeId) -> PackageDef {
    let node = db.node(id);
    // Package items are direct children of the package node in the arena
    // (see `Builder::walk_package`); filter by kind so the projection is
    // independent of child order.
    let params = node
        .children
        .iter()
        .filter_map(|c| match db.node_kind(*c) {
            db::NodeKind::Param { ty, value, local } => Some(ParamModel {
                name: db.node(*c).name.clone(),
                value: value.clone(),
                ty: ty.clone(),
                local: *local,
            }),
            _ => None,
        })
        .collect();
    let enum_consts = node
        .children
        .iter()
        .filter_map(|c| match db.node_kind(*c) {
            db::NodeKind::EnumConst { value } => Some(EnumConstDef {
                name: db.node(*c).name.clone(),
                value: value.clone(),
                file: db.node(*c).file.clone(),
                line: db.node(*c).line,
                col: db.node(*c).col,
            }),
            _ => None,
        })
        .collect();
    PackageDef {
        name: node.name.clone(),
        file: node.file.clone(),
        line: node.line,
        col: node.col,
        params,
        enum_consts,
    }
}

fn class_from_db(db: &db::Db, id: NodeId) -> ClassDef {
    let node = db.node(id);
    let scope = clean_name(&node.name).to_owned();
    let methods = node
        .children
        .iter()
        .filter_map(|c| match db.node_kind(*c) {
            db::NodeKind::FuncTask {
                is_task,
                automatic,
                ret,
            } => {
                let name = db.node(*c).name.clone();
                // `function new()` returns the implicit class handle in UHDM;
                // SystemVerilog source does not spell a return type, so the
                // constructor projects as void.
                let ret = if name == "new" { None } else { ret.clone() };
                Some(FuncDef {
                    name,
                    is_task: *is_task,
                    automatic: *automatic,
                    file: db.node(*c).file.clone(),
                    line: db.node(*c).line,
                    col: db.node(*c).col,
                    ret,
                    args: func_args_from_db(db, *c),
                    // Class definitions are not per-instance; the method scope
                    // is the (library-stripped) class name.
                    scope: scope.clone(),
                })
            }
            _ => None,
        })
        .collect();
    let fields = node
        .children
        .iter()
        .filter_map(|c| match db.node_kind(*c) {
            db::NodeKind::Var { ty } | db::NodeKind::Array { ty } => Some(ClassFieldDef {
                name: db.node(*c).name.clone(),
                ty: ty.clone(),
                line: db.node(*c).line,
                col: db.node(*c).col,
            }),
            _ => None,
        })
        .collect();
    ClassDef {
        name: node.name.clone(),
        file: node.file.clone(),
        line: node.line,
        col: node.col,
        methods,
        fields,
    }
}

/// Strip a Surelog library prefix (`lib@name` → `name`) from a design name.
fn clean_name(name: &str) -> &str {
    match name.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest,
        _ => name,
    }
}

fn port_from_db(db: &db::Db, id: NodeId) -> PortModel {
    let node = db.node(id);
    let (direction, low) = match &node.kind {
        db::NodeKind::Port { direction, low, .. } => (*direction, *low),
        _ => (Direction::None, None),
    };
    // The port's type is the type of the net/var its low connection binds to
    // (the child-side signal), which the database resolved during the walk.
    let ty = low
        .and_then(|lid| signal_type_of(db, lid))
        .unwrap_or_default();
    PortModel {
        name: node.name.clone(),
        direction,
        ty,
    }
}

fn signal_type_of(db: &db::Db, id: NodeId) -> Option<TypeInfo> {
    match db.node_kind(id) {
        db::NodeKind::Net { ty, .. } | db::NodeKind::Var { ty } | db::NodeKind::Array { ty } => {
            Some(ty.clone())
        }
        _ => None,
    }
}

fn gen_scopes_from_db(db: &db::Db, gsa_id: NodeId) -> Vec<GenScopeModel> {
    let gsa = db.node(gsa_id);
    gsa.children
        .iter()
        .filter(|c| matches!(db.node_kind(**c), db::NodeKind::GenScope))
        .map(|g| {
            let scope = db.node(*g);
            let name = if scope.name.is_empty() {
                gsa.name.clone()
            } else {
                scope.name.clone()
            };
            let params = scope
                .children
                .iter()
                .filter_map(|c| match db.node_kind(*c) {
                    db::NodeKind::Param { ty, value, local } => Some(ParamModel {
                        name: db.node(*c).name.clone(),
                        value: value.clone(),
                        ty: ty.clone(),
                        local: *local,
                    }),
                    _ => None,
                })
                .collect();
            let children = scope
                .children
                .iter()
                .filter(|&c| matches!(db.node_kind(*c), db::NodeKind::ModuleInst { .. }))
                .map(|c| instance_from_db(db, *c))
                .collect();
            GenScopeModel {
                name,
                full_name: if scope.full_name.is_empty() {
                    gsa.full_name.clone()
                } else {
                    scope.full_name.clone()
                },
                params,
                children,
            }
        })
        .collect()
}

// ── Typespec handling ─────────────────────────────────────────────────────────

/// Compute `(width, signed)` from a typespec handle, following `ref_typespec`
/// `vpiActual` chains (guarded against cycles).  `None` when the width cannot
/// be determined (unsized/unknown, arrays, unresolvable range bounds, …).
///
/// Exposed for future codegen use.
pub fn typespec_info(ts: VpiHandle) -> Option<(u32, bool)> {
    let mut visited: HashSet<(i32, String)> = HashSet::new();
    let mut current: Option<OwnedHandle> = None;
    loop {
        let cur = current.as_ref().map_or(ts, OwnedHandle::raw);
        let t = vpi::obj_type(cur);
        // `vpiType` reports the *VPI-mapped* type (e.g. vpiRefTypespec), not
        // the raw UHDM discriminant.
        if t == vpi::vpiRefTypespec {
            if !visited.insert((t, vpi::obj_full_name(cur))) {
                return None;
            }
            current = Some(match current.as_ref() {
                Some(owner) => owner.child(vpi::vpiActual)?,
                None => vpi::handle(vpi::vpiActual, ts)?,
            });
            continue;
        }
        return concrete_typespec_info(cur, t);
    }
}

/// Width/signedness of a concrete (non-ref) typespec object.
fn concrete_typespec_info(ts: VpiHandle, t: i32) -> Option<(u32, bool)> {
    let signed = vpi::get(vpi::vpiSigned, ts) != 0;
    match t {
        vpi::vpiIntTypespec | vpi::vpiIntegerTypespec | vpi::vpiTimeTypespec => Some((32, signed)),
        vpi::vpiLongIntTypespec => Some((64, signed)),
        vpi::vpiByteTypespec => Some((8, true)),
        vpi::vpiShortIntTypespec => Some((16, signed)),
        vpi::vpiLogicTypespec | vpi::vpiBitTypespec | vpi::vpiPackedArrayTypespec => {
            range_width(ts).map(|w| (w, signed))
        }
        vpi::vpiEnumTypespec => {
            let base = vpi::handle(vpi::vpiBaseTypespec, ts)?;
            typespec_info(base.raw())
        }
        vpi::vpiStructTypespec | vpi::vpiUnionTypespec => struct_width(ts).map(|w| (w, signed)),
        // Arrays, strings, reals, classes and unknown types are v1-unsized.
        _ => None,
    }
}

/// Packed width of a logic/bit typespec: product of `|left - right| + 1`
/// across all `vpiRange`s; 1 when there is no range.
fn range_width(ts: VpiHandle) -> Option<u32> {
    let mut total: u64 = 1;
    let mut any = false;
    for r in iter(vpi::vpiRange, ts) {
        any = true;
        let l = vpi::handle(vpi::vpiLeftRange, r.raw());
        let rr = vpi::handle(vpi::vpiRightRange, r.raw());
        // Borrow the OwnedHandles so they stay alive while the raw bound
        // handles below are read (a raw pointer from a dropped OwnedHandle
        // would have been released already).
        let (lh, rh) = match (&l, &rr) {
            (Some(l), Some(r)) => (l.raw(), r.raw()),
            _ => return None,
        };
        let lv = const_i128(lh)?;
        let rv = const_i128(rh)?;
        let dim = (lv - rv).abs() + 1;
        if dim <= 0 || dim > (1 << 24) {
            return None;
        }
        total = total.saturating_mul(dim as u64);
    }
    if !any {
        return Some(1);
    }
    if total == 0 || total > (1 << 24) {
        None
    } else {
        Some(total as u32)
    }
}

/// Sum of member widths for a struct/union typespec.
fn struct_width(ts: VpiHandle) -> Option<u32> {
    let mut total: u64 = 0;
    let mut any = false;
    for m in iter(vpi::vpiTypespecMember, ts) {
        let mts = vpi::handle(vpi::vpiTypespec, m.raw())?;
        let (w, _) = typespec_info(mts.raw())?;
        total = total.saturating_add(w as u64);
        any = true;
    }
    if !any {
        return None;
    }
    if total > (1 << 24) {
        None
    } else {
        Some(total as u32)
    }
}

/// Read a range-bound constant as an integer.  Elaborated output folds every
/// bound to a `constant`; anything else (operations, X/Z) yields `None`.
fn const_i128(h: VpiHandle) -> Option<i128> {
    match vpi::read_value(h) {
        ValueData::Int(v) => Some(v as i128),
        ValueData::UInt(v) => Some(v as i128),
        ValueData::Scalar(v) => Some(v as i128),
        _ => None,
    }
}

/// Collect owned child handles of a 1-to-many relationship.
fn iter(type_: i32, obj: VpiHandle) -> Vec<OwnedHandle> {
    vpi::iterate(type_, obj)
        .map(|it| it.collect())
        .unwrap_or_default()
}
