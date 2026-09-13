//! model — owned design projection over the Slang semantic database, shared by the LSP
//! (symbols, hover, completion) and the simulator (codegen input).
//!
//! [`DesignModel`] is fully owned, has no frontend lifetime, and is `Send + Sync`.
//! It is shaped from [`core::db::Db`] after the native Slang snapshot has been
//! copied and validated; no native pointer or frontend lifetime is retained.

use std::collections::HashSet;

use crate::core::db::{self, NodeId};
use crate::core::elab::Val;

/// Direction of a semantic port.
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
    /// `"union"`, `"array"`, `"string"`, `"chandle"`, `"class"`, `"other"`.
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
    /// `true` when the formal is a read-only `const ref` alias.
    pub const_ref: bool,
    /// `true` when the formal uses the explicit `ref static` lifetime.
    pub ref_static: bool,
}

/// A function or task definition captured in an elaborated instance scope.
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
    /// Owned DPI-C import metadata, when this declaration is foreign.
    pub dpi_import: Option<DpiImportDef>,
    /// Full name of the instance the clone belongs to.
    pub scope: String,
}

/// C linkage and optimizer qualifiers for a DPI-C import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpiImportDef {
    pub c_name: String,
    pub context: bool,
    pub pure: bool,
}

/// One elaborated generate block instance, e.g. `gen_blk[0]`.
#[derive(Debug, Clone, PartialEq)]
pub struct GenScopeModel {
    pub name: String,
    /// Semantic full name including the concrete generate iteration.
    pub full_name: String,
    /// Gen-scope parameters (genvars) with their concrete values.
    pub params: Vec<ParamModel>,
    /// Module/interface instances elaborated inside this generate scope.
    /// Nested generate scopes are intentionally not flattened here; the
    /// direct children are enough for hierarchy consumers to preserve the
    /// generated boundary without inventing source-level structure.
    pub children: Vec<InstanceModel>,
}

/// One elaborated module or interface instance.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceModel {
    pub name: String,
    pub def_name: String,
    /// Semantic hierarchical name; falls back to `name` for the top instance.
    pub full_name: String,
    /// Instantiation or definition source file.
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
    /// Shape an owned [`db::Db`] into the model structures.
    ///
    /// Fully owned: the returned model shares nothing with the database.
    pub fn from_db(db: &db::Db) -> DesignModel {
        let top_instances = db
            .tops()
            .iter()
            .map(|id| instance_from_db(db, *id))
            .collect();
        let modules = db
            .flat_modules()
            .iter()
            .map(|id| module_def_from_db(db, *id))
            .collect();
        let packages = db
            .packages()
            .iter()
            .map(|id| package_from_db(db, *id))
            .collect();
        let classes = db
            .classes()
            .iter()
            .map(|id| class_from_db(db, *id))
            .collect();
        DesignModel {
            design_name: db.design_name().to_owned(),
            top_instances,
            modules,
            packages,
            classes,
        }
    }

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
                db::NodeKind::Net { ty, net_type, .. } => (net_kind(*net_type), ty),
                db::NodeKind::Var { ty } => ("var", ty),
                db::NodeKind::Array { ty } => ("array", ty),
                _ => return None,
            };
            let name = db.node(*c).name.as_str();
            // Signals are internal-only in the explorer, so do not duplicate
            // the port's backing declaration here.
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
            db::NodeKind::GenScope => vec![gen_scope_from_db(db, *c, None)],
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
                ..
            } => Some(FuncDef {
                name: db.node(*c).name.clone(),
                is_task: *is_task,
                automatic: *automatic,
                file: db.node(*c).file.clone(),
                line: db.node(*c).line,
                col: db.node(*c).col,
                ret: ret.clone(),
                args: func_args_from_db(db, *c),
                dpi_import: db.dpi_import(*c).map(|dpi| DpiImportDef {
                    c_name: dpi.c_name.clone(),
                    context: dpi.context,
                    pure: dpi.pure,
                }),
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
                const_ref,
                ref_static,
            } => Some(FuncArgDef {
                name: db.node(*c).name.clone(),
                direction: model_direction(*direction),
                ty: ty.clone(),
                has_default: default.is_some(),
                const_ref: *const_ref,
                ref_static: *ref_static,
            }),
            _ => None,
        })
        .collect()
}

fn net_kind(net_type: db::NetType) -> &'static str {
    match net_type {
        db::NetType::Wire => "wire",
        db::NetType::Wand => "wand",
        db::NetType::Wor => "wor",
        db::NetType::Tri => "tri",
        db::NetType::Tri0 => "tri0",
        db::NetType::Tri1 => "tri1",
        db::NetType::TriReg => "trireg",
        db::NetType::TriAnd => "triand",
        db::NetType::TriOr => "trior",
        db::NetType::Supply0 => "supply0",
        db::NetType::Supply1 => "supply1",
        db::NetType::Uwire => "uwire",
        _ => "net",
    }
}

fn model_direction(direction: db::Direction) -> Direction {
    match direction {
        db::Direction::Input => Direction::Input,
        db::Direction::Output => Direction::Output,
        db::Direction::Inout => Direction::Inout,
        db::Direction::Mixed
        | db::Direction::None
        | db::Direction::Ref
        | db::Direction::Unsupported => Direction::None,
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
    let enum_consts = db
        .node_ids()
        .filter(|candidate| is_descendant_of(db, *candidate, id))
        .filter_map(|c| match db.node_kind(c) {
            db::NodeKind::EnumConst { value } => Some(EnumConstDef {
                name: db.node(c).name.clone(),
                value: value.clone(),
                file: db.node(c).file.clone(),
                line: db.node(c).line,
                col: db.node(c).col,
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

fn is_descendant_of(db: &db::Db, mut node: NodeId, ancestor: NodeId) -> bool {
    while let Some(parent) = db.node(node).parent {
        if parent == ancestor {
            return true;
        }
        node = parent;
    }
    false
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
                ..
            } => {
                let name = db.node(*c).name.clone();
                // SystemVerilog does not spell a return type for `new`, so
                // the constructor projects as void.
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
                    dpi_import: db.dpi_import(*c).map(|dpi| DpiImportDef {
                        c_name: dpi.c_name.clone(),
                        context: dpi.context,
                        pure: dpi.pure,
                    }),
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

fn clean_name(name: &str) -> &str {
    name
}

fn port_from_db(db: &db::Db, id: NodeId) -> PortModel {
    let node = db.node(id);
    let (direction, ty, low) = match &node.kind {
        db::NodeKind::Port {
            direction, ty, low, ..
        } => (model_direction(*direction), ty.clone(), *low),
        _ => (Direction::None, TypeInfo::default(), None),
    };
    // Incomplete diagnostic snapshots can lack a direct port type but still
    // retain the child-side declaration that the port binds to.
    let ty = (ty != TypeInfo::default())
        .then_some(ty)
        .or_else(|| low.and_then(|id| signal_type_of(db, id)))
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
        .map(|g| gen_scope_from_db(db, *g, Some(gsa_id)))
        .collect()
}

fn gen_scope_from_db(db: &db::Db, scope_id: NodeId, array: Option<NodeId>) -> GenScopeModel {
    let scope = db.node(scope_id);
    let fallback = array.map(|id| db.node(id));
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
        name: if scope.name.is_empty() {
            fallback.map_or_else(String::new, |node| node.name.clone())
        } else {
            scope.name.clone()
        },
        full_name: if scope.full_name.is_empty() {
            fallback.map_or_else(String::new, |node| node.full_name.clone())
        } else {
            scope.full_name.clone()
        },
        params,
        children,
    }
}
