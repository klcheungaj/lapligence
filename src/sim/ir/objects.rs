//! Non-integral storage and expressions, kept distinct from packed vectors.

use super::{IrCallArg, IrEnumMember, IrExpr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Non-integral scalar storage categories, independent of the packed backend.
pub enum IrObjectType {
    String,
    Chandle,
    /// A SystemVerilog semaphore object.  The C representation is an opaque
    /// runtime pointer, but keeping a distinct IR type prevents accidental
    /// treatment as an ordinary user chandle during lowering and cleanup.
    Semaphore,
    /// Stable SystemVerilog `process` class handle. The pointed-to identity
    /// remains valid after coroutine completion until all copies release it.
    Process,
}

/// C layout of one nominal SystemVerilog class. Handles are represented by a
/// pointer to the emitted struct; the layout is kept in the execution IR so
/// both optimized and unoptimized renderings share one source of truth.
#[derive(Clone, Debug, PartialEq)]
pub struct IrClass {
    pub(in crate::sim) c_name: String,
    /// Base class layout index, if this is a derived class.  Emitted structs
    /// flatten the base fields in declaration order so a base receiver points
    /// at the same object prefix as its derived allocation.
    pub(in crate::sim) base: Option<usize>,
    pub(in crate::sim) fields: Vec<IrClassField>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrClassField {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) ty: IrClassFieldType,
}

/// One packed member exposed by a virtual-interface view. The member index is
/// shared by every elaborated interface instance of the same specialization;
/// each instance supplies the concrete signal address at that slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVirtualInterfaceMember {
    pub(in crate::sim) name: String,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
}

/// One concrete interface instance reachable through a virtual-interface
/// handle. `members` and `methods` are indexed by their parent descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVirtualInterfaceInstance {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) members: Vec<Option<usize>>,
    pub(in crate::sim) methods: Vec<Option<usize>>,
}

/// One interface method dispatch entry. The first function is the signature
/// representative used by call lowering; each instance entry identifies the
/// concrete implementation invoked by the generated dispatcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVirtualInterfaceMethod {
    pub(in crate::sim) name: String,
    pub(in crate::sim) function: usize,
    pub(in crate::sim) instances: Vec<Option<usize>>,
}

/// Owned runtime metadata for virtual-interface member and method dispatch.
/// Handles use the existing opaque-pointer ABI; this descriptor keeps the
/// target table in the execution IR so optimizer variants share it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVirtualInterface {
    pub(in crate::sim) identity: String,
    pub(in crate::sim) members: Vec<IrVirtualInterfaceMember>,
    pub(in crate::sim) instances: Vec<IrVirtualInterfaceInstance>,
    pub(in crate::sim) methods: Vec<IrVirtualInterfaceMethod>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrClassFieldType {
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Real {
        shortreal: bool,
    },
    String,
    Chandle,
}

#[derive(Clone, Debug, PartialEq)]
/// Persistent object storage. Strings default empty; chandles default null.
pub struct IrObject {
    /// Backend-safe unique storage identifier.
    pub c_name: String,
    pub ty: IrObjectType,
    /// String declaration initialization, evaluated before user processes.
    pub initial: Option<IrStringExpr>,
}

/// String expression results own their bytes; reading storage creates a copy.
#[derive(Clone, Debug, PartialEq)]
pub enum IrStringExpr {
    Literal(Vec<u8>),
    /// Snapshot of the current process/object random stream. The returned
    /// bytes are an owned, versioned state string consumed by set_randstate.
    RandomState,
    Read(usize),
    LocalRead(String),
    /// Read and clone a native string formal. The callee owns the returned
    /// value; the formal itself remains caller-owned for ref/output aliases.
    FormalRead(usize),
    /// Read and clone a string element from a generic dynamic array.
    ContainerGet {
        container: usize,
        index: Box<IrExpr>,
    },
    ContainerGetNested {
        container: usize,
        indices: Vec<IrExpr>,
    },
    AssociativeGet {
        container: usize,
        key: Box<IrStringExpr>,
    },
    Call {
        receiver: Option<Box<IrChandleExpr>>,
        virtual_dispatch: bool,
        function: usize,
        args: Vec<IrExpr>,
        depth: super::IrDepth,
    },
    /// Typed subroutine call used when at least one formal is a native string
    /// (or when an object-valued output/ref formal needs call-boundary
    /// ownership). Arguments use the same ABI order as [`IrCall`].
    TypedCall {
        receiver: Option<Box<IrChandleExpr>>,
        virtual_dispatch: bool,
        function: usize,
        args: Vec<super::IrCallArg>,
        depth: super::IrDepth,
    },
    Concat(Vec<IrStringExpr>),
    Repeat(Box<IrStringExpr>, Box<IrExpr>),
    FromPacked(Box<IrExpr>),
    /// Return the declaration name matching a runtime enum value.  The
    /// receiver and values remain typed packed expressions so optimizer
    /// traversal and capacity accounting see every dependency.
    EnumName {
        receiver: Box<IrExpr>,
        members: Vec<IrEnumMember>,
    },
    /// Format an owned string with the shared typed display formatter.  The
    /// format expression and every argument are evaluated in source order;
    /// the returned string owns its storage independently of all inputs.
    Format {
        format: Box<IrStringExpr>,
        args: Vec<IrDisplayArg>,
        scope: String,
    },
    Case(Box<IrStringExpr>, bool),
    Substr(Box<IrStringExpr>, Box<IrExpr>, Box<IrExpr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrStringInsideItem {
    Value(IrStringExpr),
    Range {
        low: IrStringExpr,
        high: IrStringExpr,
    },
}

/// One typed argument of a display-family task.
///
/// Display values stay in their native representation until the runtime
/// formatter consumes them. In particular, strings are not converted through
/// packed bits and real values are not passed through a variadic C call.
#[derive(Clone, Debug, PartialEq)]
pub enum IrDisplayArg {
    Packed(IrExpr),
    Real(IrExpr),
    String(IrStringExpr),
}

impl IrDisplayArg {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
        path: &str,
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::Packed(value) => {
                if value.is_real() {
                    return Err(super::IrValidationError::new(
                        path,
                        "display packed argument cannot be real",
                    ));
                }
                let _ = (model, path);
                Ok(())
            }
            Self::Real(value) => {
                if !value.is_real() {
                    return Err(super::IrValidationError::new(
                        path,
                        "display real argument is not real",
                    ));
                }
                let _ = (model, path);
                Ok(())
            }
            Self::String(value) => value.validate(model, string_return),
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Packed(value) | Self::Real(value) => visit(value),
            Self::String(value) => value.expressions(visit),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Packed(value) | Self::Real(value) => visit(value),
            Self::String(value) => value.expressions_mut(visit),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Native-pointer values; arithmetic and packed conversion are not represented.
pub enum IrChandleExpr {
    Null,
    /// A lowering-produced C fragment. Class allocation and member reads use
    /// this escape hatch until they need a richer object-expression node.
    Verbatim(String),
    Read(usize),
    LocalRead(String),
    FormalRead(usize),
    /// Read a chandle element from a generic dynamic array.
    ContainerGet {
        container: usize,
        index: Box<IrExpr>,
    },
    ContainerGetNested {
        container: usize,
        indices: Vec<IrExpr>,
    },
    AssociativeGet {
        container: usize,
        key: Box<IrStringExpr>,
    },
    Call {
        receiver: Option<Box<IrChandleExpr>>,
        virtual_dispatch: bool,
        function: usize,
        /// Arguments are stored in the generated C parameter order.  The
        /// typed variants preserve packed arguments and pointer addresses
        /// without making a chandle look like an integer.
        args: Vec<IrCallArg>,
        depth: super::IrDepth,
    },
}

/// A process-class handle expression. Process identities are deliberately
/// distinct from chandles: they carry reference-counted lifecycle state and
/// are never converted to packed values or arbitrary native pointers.
#[derive(Clone, Debug, PartialEq)]
pub enum IrProcessExpr {
    Null,
    SelfHandle,
    Read(usize),
    LocalRead(String),
    FormalRead(usize),
}

/// Element type retained by a mailbox constructor.  Mailbox messages carry
/// their own runtime tag as well, so untyped mailboxes can safely preserve
/// heterogeneous values while typed mailboxes reject incompatible calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrMailboxElement {
    Untyped,
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Real {
        shortreal: bool,
    },
    String,
    Handle,
}

/// A value copied into a mailbox. Strings are lowered as owned expressions;
/// handle values retain pointer identity while packed values are copied by
/// value.
#[derive(Clone, Debug, PartialEq)]
pub enum IrMailboxValue {
    /// Canonical nominal identity of the expression's declared type, never
    /// the dynamic type of its handle value. Structural scalars need no tag.
    Typed {
        type_id: u64,
        value: Box<IrMailboxValue>,
    },
    Packed {
        value: IrExpr,
        two_state: bool,
    },
    Real {
        value: IrExpr,
        shortreal: bool,
    },
    String(IrStringExpr),
    Handle(IrChandleExpr),
}

/// A writable mailbox destination. The address is complete C lvalue syntax
/// (for example `&G_x` or `o0`) and remains valid across a suspended call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrMailboxTarget {
    Typed {
        type_id: u64,
        target: Box<IrMailboxTarget>,
    },
    Ref {
        addr: String,
    },
    Packed {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Real {
        addr: String,
        shortreal: bool,
    },
    String {
        addr: String,
    },
    Handle {
        addr: String,
    },
}

/// Mailbox handle expressions used by assignments and identity comparisons.
#[derive(Clone, Debug, PartialEq)]
pub enum IrMailboxExpr {
    Null,
    Read(IrChandleExpr),
    New {
        bound: IrExpr,
        element: IrMailboxElement,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrProcessControl {
    Kill,
    Suspend,
    Resume,
}

/// Queries produce packed or real values, never integer encodings of objects.
#[derive(Clone, Debug, PartialEq)]
pub enum IrObjectQuery {
    StringLen(IrStringExpr),
    StringGetc(IrStringExpr, Box<IrExpr>),
    StringCompare(IrStringExpr, IrStringExpr, bool),
    StringInside {
        value: IrStringExpr,
        items: Vec<IrStringInsideItem>,
    },
    StringAtoi(IrStringExpr, u32),
    StringAtoreal(IrStringExpr),
    StringPacked(IrStringExpr),
    ChandleEq(IrChandleExpr, IrChandleExpr),
    SemaphoreTryGet(IrChandleExpr, IrExpr),
    MailboxNum(IrChandleExpr),
    MailboxTryPut {
        mailbox: IrChandleExpr,
        value: IrMailboxValue,
    },
    MailboxTryGet {
        mailbox: IrChandleExpr,
        target: IrMailboxTarget,
        peek: bool,
    },
    MailboxEq(IrMailboxExpr, IrMailboxExpr),
    ProcessEq(IrProcessExpr, IrProcessExpr),
    ProcessStatus(IrProcessExpr),
    ArrayQuery(IrArrayQuery),
}

#[derive(Clone, Debug, PartialEq)]
/// Object mutations and output. String values are copied, not storage aliases.
pub enum IrObjectStmt {
    StringPrint(IrStringExpr),
    StringAssign(usize, IrStringExpr),
    StringAssignLocal(String, IrStringExpr),
    StringPutc(usize, IrExpr, IrExpr),
    StringItoa(usize, IrExpr, u32),
    StringRealtoa(usize, IrExpr),
    StringPutcLocal(String, IrExpr, IrExpr),
    StringItoaLocal(String, IrExpr, u32),
    StringRealtoaLocal(String, IrExpr),
    /// Declare an automatic native chandle local at the source declaration.
    ChandleDeclareLocal(String, Option<IrChandleExpr>),
    ChandleAssign(usize, IrChandleExpr),
    ChandleAssignLocal(String, IrChandleExpr),
    SemaphorePut(IrChandleExpr, IrExpr),
    SemaphoreGet(IrChandleExpr, IrExpr),
    MailboxAssign(usize, IrMailboxExpr),
    MailboxAssignLocal(String, IrMailboxExpr),
    MailboxPut(usize, IrChandleExpr, IrMailboxValue, bool),
    MailboxPutLocal(String, IrChandleExpr, IrMailboxValue, bool),
    MailboxTryPut(usize, IrChandleExpr, IrMailboxValue),
    MailboxTryPutLocal(String, IrChandleExpr, IrMailboxValue),
    MailboxGet(usize, IrChandleExpr, IrMailboxTarget, bool),
    MailboxGetLocal(String, IrChandleExpr, IrMailboxTarget, bool),
    MailboxTryGet(usize, IrChandleExpr, IrMailboxTarget, bool),
    MailboxTryGetLocal(String, IrChandleExpr, IrMailboxTarget, bool),
    /// Declare an automatic process handle local. The optional initializer is
    /// restricted to `null` or `process::self()` by lowering.
    ProcessDeclareLocal(String, Option<IrProcessExpr>),
    ProcessAssign(usize, IrProcessExpr),
    ProcessAssignLocal(String, IrProcessExpr),
    ProcessControl {
        op: IrProcessControl,
        target: IrProcessExpr,
    },
    ProcessAwait(IrProcessExpr),
}

impl IrStringExpr {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        if let Self::Format {
            format,
            args,
            scope,
        } = self
        {
            if scope.is_empty() {
                return Err(super::IrValidationError::new(
                    "string format",
                    "format scope must not be empty",
                ));
            }
            format.validate(model, string_return)?;
            for arg in args {
                arg.validate(model, string_return, "string format argument")?;
            }
            return Ok(());
        }
        let mut valid = true;
        self.expressions(&mut |expr| valid &= !expr.is_real());
        if !valid {
            return Err(super::IrValidationError::new(
                "string",
                "string operation requires packed integral operands",
            ));
        }
        match self {
            Self::LocalRead(name) if name == "_ret" && string_return != Some(true) => {
                Err(super::IrValidationError::new(
                    "string local",
                    "string return storage requires its function context",
                ))
            }
            Self::LocalRead(name) if !name.is_empty() => Ok(()),
            Self::FormalRead(_) => Ok(()),
            Self::Call {
                function,
                args,
                depth,
                receiver,
                ..
            } => {
                if depth.func_base && string_return.is_none() {
                    return Err(super::IrValidationError::new(
                        "string call",
                        "function-relative call depth outside a function",
                    ));
                }
                let callee = model.funcs.get(*function).ok_or_else(|| {
                    super::IrValidationError::new("string call", "function index is out of bounds")
                })?;
                if callee.receiver_class.is_some() != receiver.is_some() {
                    return Err(super::IrValidationError::new(
                        "object call",
                        "receiver does not match callee ABI",
                    ));
                }
                if let Some(receiver) = receiver {
                    receiver.validate(model, &[], string_return.map(|_| false))?;
                }
                if !callee.ret_string
                    || callee.formals.len() != args.len()
                    || callee
                        .formals
                        .iter()
                        .any(|formal| formal.is_out || formal.chandle)
                {
                    return Err(super::IrValidationError::new(
                        "string call",
                        "callee must return string and have only packed input formals",
                    ));
                }
                if args
                    .iter()
                    .zip(&callee.formals)
                    .any(|(arg, formal)| arg.width != formal.width || arg.signed != formal.signed)
                {
                    return Err(super::IrValidationError::new(
                        "string call",
                        "input argument type disagrees with its formal",
                    ));
                }
                Ok(())
            }
            Self::TypedCall {
                function,
                args,
                depth,
                receiver,
                ..
            } => {
                if depth.func_base && string_return.is_none() {
                    return Err(super::IrValidationError::new(
                        "string call",
                        "function-relative call depth outside a function",
                    ));
                }
                let callee = model.funcs.get(*function).ok_or_else(|| {
                    super::IrValidationError::new("string call", "function index is out of bounds")
                })?;
                if callee.receiver_class.is_some() != receiver.is_some() {
                    return Err(super::IrValidationError::new(
                        "object call",
                        "receiver does not match callee ABI",
                    ));
                }
                if let Some(receiver) = receiver {
                    receiver.validate(model, &[], string_return.map(|_| false))?;
                }
                if !callee.ret_string || callee.formals.len() != args.len() {
                    return Err(super::IrValidationError::new(
                        "string call",
                        "typed string call target has an incompatible signature",
                    ));
                }
                Ok(())
            }
            Self::Read(index) => object_type(model, *index, IrObjectType::String),
            Self::ContainerGet { container, index } => {
                let Some(container) = model.containers.get(*container) else {
                    return Err(super::IrValidationError::new(
                        "string",
                        "container index is out of bounds",
                    ));
                };
                if !container.element.is_string() || index.is_real() {
                    return Err(super::IrValidationError::new(
                        "string",
                        "string container read requires a string element and integral index",
                    ));
                }
                if !matches!(
                    container.kind,
                    super::containers::IrContainerKind::Dynamic
                        | super::containers::IrContainerKind::Queue { .. }
                ) {
                    return Err(super::IrValidationError::new(
                        "string",
                        "string container read requires a dynamic array or queue",
                    ));
                }
                Ok(())
            }
            Self::ContainerGetNested { container, indices } => {
                let Some(container) = model.containers.get(*container) else {
                    return Err(super::IrValidationError::new(
                        "string",
                        "container index is out of bounds",
                    ));
                };
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !super::containers::nested_string_element(container, indices.len())
                {
                    return Err(super::IrValidationError::new(
                        "string",
                        "nested string container read has an invalid index path",
                    ));
                }
                if !matches!(
                    container.kind,
                    super::containers::IrContainerKind::Dynamic
                        | super::containers::IrContainerKind::Queue { .. }
                ) {
                    return Err(super::IrValidationError::new(
                        "string",
                        "nested string container read requires a dynamic array or queue",
                    ));
                }
                Ok(())
            }
            Self::AssociativeGet { container, key } => {
                let Some(container) = model.containers.get(*container) else {
                    return Err(super::IrValidationError::new(
                        "string",
                        "container index is out of bounds",
                    ));
                };
                if !matches!(
                    container.kind,
                    super::containers::IrContainerKind::Associative {
                        key: super::containers::IrAssocKey::String
                    }
                ) || !container.element.is_string()
                {
                    return Err(super::IrValidationError::new(
                        "string",
                        "associative string read requires a string-keyed string array",
                    ));
                }
                key.validate(model, string_return)
            }
            Self::Concat(parts) => parts
                .iter()
                .try_for_each(|part| part.validate(model, string_return)),
            Self::Repeat(value, _) | Self::Case(value, _) | Self::Substr(value, _, _) => {
                value.validate(model, string_return)
            }
            Self::EnumName { receiver, members } => {
                if receiver.is_real() || members.is_empty() {
                    return Err(super::IrValidationError::new(
                        "string",
                        "enum name requires a packed receiver and declared members",
                    ));
                }
                if members.iter().any(|member| {
                    member.name.is_empty()
                        || member.value.is_real()
                        || member.value.width != receiver.width
                        || member.value.signed != receiver.signed
                }) {
                    return Err(super::IrValidationError::new(
                        "string",
                        "enum name member type or name is invalid",
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Literal(_)
            | Self::RandomState
            | Self::Read(_)
            | Self::LocalRead(_)
            | Self::FormalRead(_) => {}
            Self::ContainerGet { index, .. } => visit(index),
            Self::ContainerGetNested { indices, .. } => indices.iter().for_each(visit),
            Self::AssociativeGet { key, .. } => key.expressions(visit),
            Self::Call { args, receiver, .. } => {
                if let Some(receiver) = receiver {
                    receiver.expressions(visit);
                }
                args.iter().for_each(visit);
            }
            Self::TypedCall { args, receiver, .. } => {
                if let Some(receiver) = receiver {
                    receiver.expressions(visit);
                }
                for arg in args {
                    match arg {
                        super::IrCallArg::StringVal(value) => value.expressions(visit),
                        super::IrCallArg::StringOutTemp {
                            init: Some(value), ..
                        } => value.expressions(visit),
                        _ => {}
                    }
                }
            }
            Self::Concat(parts) => {
                for part in parts {
                    part.expressions(visit);
                }
            }
            Self::Repeat(value, count) => {
                value.expressions(visit);
                visit(count);
            }
            Self::FromPacked(value) => visit(value),
            Self::EnumName { receiver, members } => {
                visit(receiver);
                for member in members {
                    visit(&member.value);
                }
            }
            Self::Format { format, args, .. } => {
                format.expressions(visit);
                for arg in args {
                    arg.expressions(visit);
                }
            }
            Self::Case(value, _) => value.expressions(visit),
            Self::Substr(value, first, last) => {
                value.expressions(visit);
                visit(first);
                visit(last);
            }
        }
    }
    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Literal(_)
            | Self::RandomState
            | Self::Read(_)
            | Self::LocalRead(_)
            | Self::FormalRead(_) => {}
            Self::ContainerGet { index, .. } => visit(index),
            Self::ContainerGetNested { indices, .. } => indices.iter_mut().for_each(visit),
            Self::AssociativeGet { key, .. } => key.expressions_mut(visit),
            Self::Call { args, receiver, .. } => {
                if let Some(receiver) = receiver {
                    receiver.expressions_mut(visit);
                }
                args.iter_mut().for_each(visit);
            }
            Self::TypedCall { args, receiver, .. } => {
                if let Some(receiver) = receiver {
                    receiver.expressions_mut(visit);
                }
                for arg in args {
                    match arg {
                        super::IrCallArg::StringVal(value) => value.expressions_mut(visit),
                        super::IrCallArg::StringOutTemp {
                            init: Some(value), ..
                        } => value.expressions_mut(visit),
                        _ => {}
                    }
                }
            }
            Self::Concat(parts) => {
                for part in parts {
                    part.expressions_mut(visit);
                }
            }
            Self::Repeat(value, count) => {
                value.expressions_mut(visit);
                visit(count);
            }
            Self::FromPacked(value) => visit(value),
            Self::EnumName { receiver, members } => {
                visit(receiver);
                for member in members {
                    visit(&mut member.value);
                }
            }
            Self::Format { format, args, .. } => {
                format.expressions_mut(visit);
                for arg in args {
                    arg.expressions_mut(visit);
                }
            }
            Self::Case(value, _) => value.expressions_mut(visit),
            Self::Substr(value, first, last) => {
                value.expressions_mut(visit);
                visit(first);
                visit(last);
            }
        }
    }
}

/// One declared dimension. `None` denotes a runtime-sized dimension (dynamic
/// array, queue, associative array, or string); fixed bounds are retained as
/// signed source values so direction and negative ranges remain observable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrArrayDimension {
    pub left: Option<i128>,
    pub right: Option<i128>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrArrayQueryKind {
    Left,
    Right,
    Low,
    High,
    Increment,
    Size,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrArrayQueryTarget {
    Static {
        dimensions: Vec<IrArrayDimension>,
    },
    Container {
        container: usize,
        dimensions: Vec<IrArrayDimension>,
    },
    String {
        value: IrStringExpr,
        dimensions: Vec<IrArrayDimension>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrArrayQuery {
    pub kind: IrArrayQueryKind,
    pub target: IrArrayQueryTarget,
    /// None means the LRM default dimension of one.
    pub dimension: Option<Box<IrExpr>>,
}

impl IrArrayQuery {
    pub(in crate::sim) fn result_type(&self, model: &super::IrModel) -> (u32, bool) {
        if self.kind == IrArrayQueryKind::Increment {
            return (32, true);
        }
        if let IrArrayQueryTarget::Container { container, .. } = &self.target {
            if let Some(container) = model.containers.get(*container) {
                if let super::IrContainerKind::Associative {
                    key: super::IrAssocKey::Integral { width, signed, .. },
                } = &container.kind
                {
                    return (*width, *signed);
                }
            }
        }
        (32, true)
    }

    fn dimensions(&self) -> &[IrArrayDimension] {
        match &self.target {
            IrArrayQueryTarget::Static { dimensions }
            | IrArrayQueryTarget::Container { dimensions, .. }
            | IrArrayQueryTarget::String { dimensions, .. } => dimensions,
        }
    }

    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        if self.dimensions().is_empty() {
            return Err(super::IrValidationError::new(
                "array query",
                "query target has no dimensions",
            ));
        }
        if self
            .dimension
            .as_ref()
            .is_some_and(|dimension| dimension.is_real())
        {
            return Err(super::IrValidationError::new(
                "array query",
                "dimension selector must be integral",
            ));
        }
        match &self.target {
            IrArrayQueryTarget::Static { .. } => Ok(()),
            IrArrayQueryTarget::Container { container, .. } => {
                if model.containers.get(*container).is_none() {
                    return Err(super::IrValidationError::new(
                        "array query",
                        "container index is out of bounds",
                    ));
                }
                Ok(())
            }
            IrArrayQueryTarget::String { value, .. } => value.validate(model, string_return),
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        if let Some(dimension) = &self.dimension {
            visit(dimension);
        }
        if let IrArrayQueryTarget::String { value, .. } = &self.target {
            value.expressions(visit);
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        if let Some(dimension) = &mut self.dimension {
            visit(dimension);
        }
        if let IrArrayQueryTarget::String { value, .. } = &mut self.target {
            value.expressions_mut(visit);
        }
    }
}

impl IrProcessExpr {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        formals: &[super::IrFormal],
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::Null | Self::SelfHandle => Ok(()),
            Self::Read(index) => object_type(model, *index, IrObjectType::Process),
            Self::LocalRead(name) if !name.is_empty() => Ok(()),
            Self::LocalRead(_) => Err(super::IrValidationError::new(
                "process local",
                "local name must not be empty",
            )),
            Self::FormalRead(index) if *index < formals.len() => Ok(()),
            Self::FormalRead(_) => Err(super::IrValidationError::new(
                "process formal",
                "formal index is out of bounds",
            )),
        }
    }
}

impl IrMailboxValue {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        formals: &[super::IrFormal],
        chandle_return: Option<bool>,
        string_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::Packed { value, .. } if value.is_real() => Err(super::IrValidationError::new(
                "mailbox value",
                "packed mailbox value cannot be real",
            )),
            Self::Packed { .. } => Ok(()),
            Self::Real { value, .. } if !value.is_real() => Err(super::IrValidationError::new(
                "mailbox value",
                "real mailbox value must be real",
            )),
            Self::Real { .. } => Ok(()),
            Self::String(value) => value.validate(model, string_return),
            Self::Handle(value) => value.validate(model, formals, chandle_return),
            Self::Typed { type_id, value } => {
                if *type_id == 0 {
                    return Err(super::IrValidationError::new(
                        "mailbox value",
                        "nominal identity must be nonzero",
                    ));
                }
                value.validate(model, formals, chandle_return, string_return)
            }
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Packed { value, .. } | Self::Real { value, .. } => visit(value),
            Self::String(value) => value.expressions(visit),
            Self::Handle(value) => value.expressions(visit),
            Self::Typed { value, .. } => value.expressions(visit),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Packed { value, .. } | Self::Real { value, .. } => visit(value),
            Self::String(value) => value.expressions_mut(visit),
            Self::Handle(value) => value.expressions_mut(visit),
            Self::Typed { value, .. } => value.expressions_mut(visit),
        }
    }
}

impl IrMailboxTarget {
    pub(in crate::sim) fn validate(&self) -> Result<(), super::IrValidationError> {
        if let Self::Typed { type_id, target } = self {
            if *type_id == 0 {
                return Err(super::IrValidationError::new(
                    "mailbox target",
                    "nominal identity must be nonzero",
                ));
            }
            return target.validate();
        }
        let (addr, width) = match self {
            Self::Packed { addr, width, .. } => (addr, Some(*width)),
            Self::Ref { addr } => (addr, None),
            Self::Typed { .. } => unreachable!("handled above"),
            Self::Real { addr, .. } | Self::String { addr } | Self::Handle { addr } => (addr, None),
        };
        if addr.is_empty() || width == Some(0) {
            return Err(super::IrValidationError::new(
                "mailbox target",
                "writable target address and packed width must be non-empty",
            ));
        }
        Ok(())
    }
}

impl IrMailboxExpr {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        formals: &[super::IrFormal],
        chandle_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::Null => Ok(()),
            Self::Read(value) => value.validate(model, formals, chandle_return),
            Self::New { bound, element } => {
                if bound.is_real() {
                    return Err(super::IrValidationError::new(
                        "mailbox constructor",
                        "mailbox bound must be integral",
                    ));
                }
                if let IrMailboxElement::Packed { width, .. } = element {
                    if *width == 0 {
                        return Err(super::IrValidationError::new(
                            "mailbox constructor",
                            "mailbox packed element width must be non-zero",
                        ));
                    }
                }
                Ok(())
            }
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Read(value) => value.expressions(visit),
            Self::New { bound, .. } => visit(bound),
            Self::Null => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Read(value) => value.expressions_mut(visit),
            Self::New { bound, .. } => visit(bound),
            Self::Null => {}
        }
    }
}

impl IrObjectQuery {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        formals: &[super::IrFormal],
        chandle_return: Option<bool>,
        string_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::StringAtoi(_, base) if !matches!(base, 2 | 8 | 10 | 16) => Err(
                super::IrValidationError::new("string", "invalid numeric base"),
            ),
            Self::StringLen(value)
            | Self::StringGetc(value, _)
            | Self::StringAtoi(value, _)
            | Self::StringAtoreal(value)
            | Self::StringPacked(value) => value.validate(model, string_return),
            Self::StringCompare(a, b, _) => {
                a.validate(model, string_return)?;
                b.validate(model, string_return)
            }
            Self::StringInside { value, items } => {
                if items.is_empty() {
                    return Err(super::IrValidationError::new(
                        "string",
                        "string inside set is empty",
                    ));
                }
                value.validate(model, string_return)?;
                for item in items {
                    match item {
                        IrStringInsideItem::Value(value) => value.validate(model, string_return)?,
                        IrStringInsideItem::Range { low, high } => {
                            low.validate(model, string_return)?;
                            high.validate(model, string_return)?;
                        }
                    }
                }
                Ok(())
            }
            Self::ChandleEq(a, b) => {
                a.validate(model, formals, chandle_return)?;
                b.validate(model, formals, chandle_return)
            }
            Self::SemaphoreTryGet(receiver, keys) => {
                receiver.validate(model, formals, chandle_return)?;
                if keys.is_real() {
                    Err(super::IrValidationError::new(
                        "semaphore",
                        "semaphore key count must be integral",
                    ))
                } else {
                    Ok(())
                }
            }
            Self::MailboxNum(mailbox) => mailbox.validate(model, formals, chandle_return),
            Self::MailboxTryPut { mailbox, value } => {
                mailbox.validate(model, formals, chandle_return)?;
                value.validate(model, formals, chandle_return, string_return)
            }
            Self::MailboxTryGet {
                mailbox, target, ..
            } => {
                mailbox.validate(model, formals, chandle_return)?;
                target.validate()
            }
            Self::MailboxEq(a, b) => {
                a.validate(model, formals, chandle_return)?;
                b.validate(model, formals, chandle_return)
            }
            Self::ProcessEq(a, b) => {
                a.validate(model, formals)?;
                b.validate(model, formals)
            }
            Self::ProcessStatus(value) => value.validate(model, formals),
            Self::ArrayQuery(query) => query.validate(model, string_return),
        }
    }
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::StringLen(value)
            | Self::StringAtoi(value, _)
            | Self::StringAtoreal(value)
            | Self::StringPacked(value) => value.expressions(visit),
            Self::StringGetc(value, index) => {
                value.expressions(visit);
                visit(index);
            }
            Self::StringCompare(a, b, _) => {
                a.expressions(visit);
                b.expressions(visit);
            }
            Self::StringInside { value, items } => {
                value.expressions(visit);
                for item in items {
                    match item {
                        IrStringInsideItem::Value(value) => value.expressions(visit),
                        IrStringInsideItem::Range { low, high } => {
                            low.expressions(visit);
                            high.expressions(visit);
                        }
                    }
                }
            }
            Self::ChandleEq(a, b) => {
                a.expressions(visit);
                b.expressions(visit);
            }
            Self::SemaphoreTryGet(receiver, keys) => {
                receiver.expressions(visit);
                visit(keys);
            }
            Self::MailboxNum(mailbox) => mailbox.expressions(visit),
            Self::MailboxTryPut { mailbox, value } => {
                mailbox.expressions(visit);
                value.expressions(visit);
            }
            Self::MailboxTryGet { mailbox, .. } => mailbox.expressions(visit),
            Self::MailboxEq(a, b) => {
                a.expressions(visit);
                b.expressions(visit);
            }
            Self::ProcessEq(_, _) => {}
            Self::ProcessStatus(_) => {}
            Self::ArrayQuery(query) => query.expressions(visit),
        }
    }
    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::StringLen(value)
            | Self::StringAtoi(value, _)
            | Self::StringAtoreal(value)
            | Self::StringPacked(value) => value.expressions_mut(visit),
            Self::StringGetc(value, index) => {
                value.expressions_mut(visit);
                visit(index);
            }
            Self::StringCompare(a, b, _) => {
                a.expressions_mut(visit);
                b.expressions_mut(visit);
            }
            Self::StringInside { value, items } => {
                value.expressions_mut(visit);
                for item in items {
                    match item {
                        IrStringInsideItem::Value(value) => value.expressions_mut(visit),
                        IrStringInsideItem::Range { low, high } => {
                            low.expressions_mut(visit);
                            high.expressions_mut(visit);
                        }
                    }
                }
            }
            Self::ChandleEq(a, b) => {
                a.expressions_mut(visit);
                b.expressions_mut(visit);
            }
            Self::SemaphoreTryGet(receiver, keys) => {
                receiver.expressions_mut(visit);
                visit(keys);
            }
            Self::MailboxNum(mailbox) => mailbox.expressions_mut(visit),
            Self::MailboxTryPut { mailbox, value } => {
                mailbox.expressions_mut(visit);
                value.expressions_mut(visit);
            }
            Self::MailboxTryGet { mailbox, .. } => mailbox.expressions_mut(visit),
            Self::MailboxEq(a, b) => {
                a.expressions_mut(visit);
                b.expressions_mut(visit);
            }
            Self::ProcessEq(_, _) => {}
            Self::ProcessStatus(_) => {}
            Self::ArrayQuery(query) => query.expressions_mut(visit),
        }
    }
}

impl IrObjectStmt {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        formals: &[super::IrFormal],
        chandle_return: Option<bool>,
        string_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::StringPrint(value) => value.validate(model, string_return),
            Self::StringItoa(_, _, base) if !matches!(base, 2 | 8 | 10 | 16) => Err(
                super::IrValidationError::new("string", "invalid numeric base"),
            ),
            Self::StringAssign(index, value) => {
                object_type(model, *index, IrObjectType::String)?;
                value.validate(model, string_return)
            }
            Self::StringAssignLocal(name, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "string local",
                        "local name must not be empty",
                    ));
                }
                value.validate(model, string_return)
            }
            Self::StringPutcLocal(name, index, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "string local",
                        "local name must not be empty",
                    ));
                }
                let _ = (index, model, formals);
                let _ = value;
                Ok(())
            }
            Self::StringItoaLocal(name, value, base) => {
                if name.is_empty() || !matches!(base, 2 | 8 | 10 | 16) {
                    return Err(super::IrValidationError::new(
                        "string local",
                        "invalid local string conversion",
                    ));
                }
                let _ = value;
                Ok(())
            }
            Self::StringRealtoaLocal(name, value) => {
                if name.is_empty() || !value.is_real() {
                    return Err(super::IrValidationError::new(
                        "string local",
                        "realtoa requires a named string local and real argument",
                    ));
                }
                Ok(())
            }
            Self::StringPutc(index, _, _) | Self::StringItoa(index, _, _) => {
                object_type(model, *index, IrObjectType::String)
            }
            Self::StringRealtoa(index, value) => {
                object_type(model, *index, IrObjectType::String)?;
                if !value.is_real() {
                    return Err(super::IrValidationError::new(
                        "string",
                        "realtoa requires a real argument",
                    ));
                }
                Ok(())
            }
            Self::ChandleDeclareLocal(name, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "chandle local",
                        "local name must not be empty",
                    ));
                }
                value
                    .as_ref()
                    .map(|value| value.validate(model, formals, chandle_return))
                    .unwrap_or(Ok(()))
            }
            Self::ChandleAssign(index, value) => {
                pointer_object_type(model, *index)?;
                value.validate(model, formals, chandle_return)
            }
            Self::ChandleAssignLocal(name, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "chandle local",
                        "local name must not be empty",
                    ));
                }
                value.validate(model, formals, chandle_return)
            }
            Self::SemaphorePut(receiver, keys) | Self::SemaphoreGet(receiver, keys) => {
                receiver.validate(model, formals, chandle_return)?;
                if keys.is_real() {
                    Err(super::IrValidationError::new(
                        "semaphore",
                        "semaphore key count must be integral",
                    ))
                } else {
                    Ok(())
                }
            }
            Self::MailboxAssign(index, value) => {
                object_type(model, *index, IrObjectType::Chandle)?;
                value.validate(model, formals, chandle_return)
            }
            Self::MailboxAssignLocal(name, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "mailbox local",
                        "local name must not be empty",
                    ));
                }
                value.validate(model, formals, chandle_return)
            }
            Self::MailboxPut(index, mailbox, value, _try) => {
                object_type(model, *index, IrObjectType::Chandle)?;
                mailbox.validate(model, formals, chandle_return)?;
                value.validate(model, formals, chandle_return, string_return)
            }
            Self::MailboxTryPut(index, mailbox, value) => {
                object_type(model, *index, IrObjectType::Chandle)?;
                mailbox.validate(model, formals, chandle_return)?;
                value.validate(model, formals, chandle_return, string_return)
            }
            Self::MailboxGet(index, mailbox, target, _try) => {
                object_type(model, *index, IrObjectType::Chandle)?;
                mailbox.validate(model, formals, chandle_return)?;
                target.validate()
            }
            Self::MailboxTryGet(index, mailbox, target, _peek) => {
                object_type(model, *index, IrObjectType::Chandle)?;
                mailbox.validate(model, formals, chandle_return)?;
                target.validate()
            }
            Self::MailboxPutLocal(name, mailbox, value, _try) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "mailbox local",
                        "local name must not be empty",
                    ));
                }
                mailbox.validate(model, formals, chandle_return)?;
                value.validate(model, formals, chandle_return, string_return)
            }
            Self::MailboxTryPutLocal(name, mailbox, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "mailbox local",
                        "local name must not be empty",
                    ));
                }
                mailbox.validate(model, formals, chandle_return)?;
                value.validate(model, formals, chandle_return, string_return)
            }
            Self::MailboxGetLocal(name, mailbox, target, _try) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "mailbox local",
                        "local name must not be empty",
                    ));
                }
                mailbox.validate(model, formals, chandle_return)?;
                target.validate()
            }
            Self::MailboxTryGetLocal(name, mailbox, target, _peek) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "mailbox local",
                        "local name must not be empty",
                    ));
                }
                mailbox.validate(model, formals, chandle_return)?;
                target.validate()
            }
            Self::ProcessDeclareLocal(name, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "process local",
                        "local name must not be empty",
                    ));
                }
                value
                    .as_ref()
                    .map(|value| value.validate(model, formals))
                    .unwrap_or(Ok(()))
            }
            Self::ProcessAssign(index, value) => {
                object_type(model, *index, IrObjectType::Process)?;
                value.validate(model, formals)
            }
            Self::ProcessAssignLocal(name, value) => {
                if name.is_empty() {
                    return Err(super::IrValidationError::new(
                        "process local",
                        "local name must not be empty",
                    ));
                }
                value.validate(model, formals)
            }
            Self::ProcessControl { target, .. } | Self::ProcessAwait(target) => {
                target.validate(model, formals)
            }
        }
    }
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::StringAssign(_, value)
            | Self::StringAssignLocal(_, value)
            | Self::StringPrint(value) => value.expressions(visit),
            Self::StringPutc(_, index, value) => {
                visit(index);
                visit(value);
            }
            Self::StringItoa(_, value, _) | Self::StringRealtoa(_, value) => visit(value),
            Self::StringPutcLocal(_, index, value) => {
                visit(index);
                visit(value);
            }
            Self::StringItoaLocal(_, value, _) | Self::StringRealtoaLocal(_, value) => visit(value),
            Self::ChandleDeclareLocal(_, Some(value)) => value.expressions(visit),
            Self::SemaphorePut(receiver, keys) | Self::SemaphoreGet(receiver, keys) => {
                receiver.expressions(visit);
                visit(keys);
            }
            Self::ChandleDeclareLocal(_, None)
            | Self::ChandleAssign(..)
            | Self::ChandleAssignLocal(..)
            | Self::MailboxGet(..)
            | Self::MailboxGetLocal(..)
            | Self::ProcessDeclareLocal(..)
            | Self::ProcessAssign(..)
            | Self::ProcessAssignLocal(..)
            | Self::ProcessControl { .. }
            | Self::ProcessAwait(..) => {}
            Self::MailboxAssign(_, value) | Self::MailboxAssignLocal(_, value) => {
                value.expressions(visit)
            }
            Self::MailboxPut(_, mailbox, value, _)
            | Self::MailboxPutLocal(_, mailbox, value, _) => {
                mailbox.expressions(visit);
                value.expressions(visit);
            }
            Self::MailboxTryPut(_, mailbox, value)
            | Self::MailboxTryPutLocal(_, mailbox, value) => {
                mailbox.expressions(visit);
                value.expressions(visit);
            }
            Self::MailboxTryGet(_, mailbox, _, _) | Self::MailboxTryGetLocal(_, mailbox, _, _) => {
                mailbox.expressions(visit)
            }
        }
    }
    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::StringAssign(_, value)
            | Self::StringAssignLocal(_, value)
            | Self::StringPrint(value) => value.expressions_mut(visit),
            Self::StringPutc(_, index, value) => {
                visit(index);
                visit(value);
            }
            Self::StringItoa(_, value, _) | Self::StringRealtoa(_, value) => visit(value),
            Self::StringPutcLocal(_, index, value) => {
                visit(index);
                visit(value);
            }
            Self::StringItoaLocal(_, value, _) | Self::StringRealtoaLocal(_, value) => visit(value),
            Self::ChandleDeclareLocal(_, Some(value)) => value.expressions_mut(visit),
            Self::SemaphorePut(receiver, keys) | Self::SemaphoreGet(receiver, keys) => {
                receiver.expressions_mut(visit);
                visit(keys);
            }
            Self::ChandleDeclareLocal(_, None)
            | Self::ChandleAssign(..)
            | Self::ChandleAssignLocal(..)
            | Self::MailboxGet(..)
            | Self::MailboxGetLocal(..)
            | Self::ProcessDeclareLocal(..)
            | Self::ProcessAssign(..)
            | Self::ProcessAssignLocal(..)
            | Self::ProcessControl { .. }
            | Self::ProcessAwait(..) => {}
            Self::MailboxAssign(_, value) | Self::MailboxAssignLocal(_, value) => {
                value.expressions_mut(visit)
            }
            Self::MailboxPut(_, mailbox, value, _)
            | Self::MailboxPutLocal(_, mailbox, value, _) => {
                mailbox.expressions_mut(visit);
                value.expressions_mut(visit);
            }
            Self::MailboxTryPut(_, mailbox, value)
            | Self::MailboxTryPutLocal(_, mailbox, value) => {
                mailbox.expressions_mut(visit);
                value.expressions_mut(visit);
            }
            Self::MailboxTryGet(_, mailbox, _, _) | Self::MailboxTryGetLocal(_, mailbox, _, _) => {
                mailbox.expressions_mut(visit)
            }
        }
    }
}

impl IrChandleExpr {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        formals: &[super::IrFormal],
        chandle_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::Null => Ok(()),
            Self::Verbatim(code) if !code.is_empty() => Ok(()),
            Self::Verbatim(_) => Err(super::IrValidationError::new(
                "chandle verbatim",
                "verbatim pointer expression must not be empty",
            )),
            Self::LocalRead(name) if name == "_ret" && chandle_return == Some(true) => Ok(()),
            Self::LocalRead(name) if !name.is_empty() => Ok(()),
            Self::LocalRead(_) => Err(super::IrValidationError::new(
                "chandle local",
                "local name must not be empty",
            )),
            Self::FormalRead(index) => {
                if formals.get(*index).is_some_and(|formal| formal.chandle) {
                    Ok(())
                } else {
                    Err(super::IrValidationError::new(
                        "chandle formal",
                        "index does not refer to a chandle formal",
                    ))
                }
            }
            Self::ContainerGet { container, index } => {
                let Some(container) = model.containers.get(*container) else {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "container index is out of bounds",
                    ));
                };
                if !container.element.is_chandle() || index.is_real() {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "chandle container read requires a chandle element and integral index",
                    ));
                }
                if !matches!(
                    container.kind,
                    super::containers::IrContainerKind::Dynamic
                        | super::containers::IrContainerKind::Queue { .. }
                ) {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "chandle container read requires a dynamic array or queue",
                    ));
                }
                Ok(())
            }
            Self::ContainerGetNested { container, indices } => {
                let Some(container) = model.containers.get(*container) else {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "container index is out of bounds",
                    ));
                };
                if indices.is_empty()
                    || indices.iter().any(IrExpr::is_real)
                    || !super::containers::nested_chandle_element(container, indices.len())
                {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "nested chandle container read has an invalid index path",
                    ));
                }
                if !matches!(
                    container.kind,
                    super::containers::IrContainerKind::Dynamic
                        | super::containers::IrContainerKind::Queue { .. }
                ) {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "nested chandle container read requires a dynamic array or queue",
                    ));
                }
                Ok(())
            }
            Self::AssociativeGet { container, key } => {
                let Some(container) = model.containers.get(*container) else {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "container index is out of bounds",
                    ));
                };
                if !matches!(
                    container.kind,
                    super::containers::IrContainerKind::Associative {
                        key: super::containers::IrAssocKey::String
                    }
                ) || !container.element.is_chandle()
                {
                    return Err(super::IrValidationError::new(
                        "chandle",
                        "associative chandle read requires a string-keyed chandle array",
                    ));
                }
                key.validate(model, None)
            }
            Self::Read(index) => pointer_object_type(model, *index),
            Self::Call {
                function,
                args,
                depth,
                receiver,
                ..
            } => {
                if depth.func_base && chandle_return.is_none() {
                    return Err(super::IrValidationError::new(
                        "chandle call",
                        "function-relative call depth outside a function",
                    ));
                }
                let callee = model.funcs.get(*function).ok_or_else(|| {
                    super::IrValidationError::new("chandle call", "function index is out of bounds")
                })?;
                if callee.receiver_class.is_some() != receiver.is_some() {
                    return Err(super::IrValidationError::new(
                        "object call",
                        "receiver does not match callee ABI",
                    ));
                }
                if let Some(receiver) = receiver {
                    receiver.validate(model, formals, chandle_return)?;
                }
                if !callee.ret_chandle || callee.formals.len() != args.len() {
                    return Err(super::IrValidationError::new(
                        "chandle call",
                        "callee must return chandle and have one typed argument per formal",
                    ));
                }
                let parameter_order = callee
                    .formals
                    .iter()
                    .filter(|formal| formal.is_address())
                    .chain(callee.formals.iter().filter(|formal| !formal.is_address()));
                for (arg, formal) in args.iter().zip(parameter_order) {
                    match (arg, formal) {
                        (IrCallArg::ChandleVal(value), formal)
                            if formal.chandle && !formal.is_address() =>
                        {
                            value.validate(model, formals, chandle_return)?;
                        }
                        (IrCallArg::Val(value), formal)
                            if !formal.chandle && !formal.is_address() =>
                        {
                            if value.is_real() != formal.real
                                || value.width != formal.width
                                || value.signed != formal.signed
                            {
                                return Err(super::IrValidationError::new(
                                    "chandle call",
                                    "packed argument type disagrees with its formal",
                                ));
                            }
                        }
                        (IrCallArg::ChandleAddr(addr), formal)
                            if formal.chandle && formal.is_out && !formal.is_ref() =>
                        {
                            if addr.is_empty() {
                                return Err(super::IrValidationError::new(
                                    "chandle call",
                                    "output address must not be empty",
                                ));
                            }
                        }
                        (IrCallArg::ChandleRefAddr(addr), formal)
                            if formal.chandle && formal.is_ref() =>
                        {
                            if addr.is_empty() {
                                return Err(super::IrValidationError::new(
                                    "chandle call",
                                    "reference address must not be empty",
                                ));
                            }
                        }
                        _ => {
                            return Err(super::IrValidationError::new(
                                "chandle call",
                                "argument kind does not match its chandle signature",
                            ));
                        }
                    }
                }
                Ok(())
            }
        }
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::ContainerGet { index, .. } => visit(index),
            Self::ContainerGetNested { indices, .. } => indices.iter().for_each(visit),
            Self::Call { args, receiver, .. } => {
                if let Some(receiver) = receiver {
                    receiver.expressions(visit);
                }
                for arg in args {
                    match arg {
                        IrCallArg::Val(value) => visit(value),
                        IrCallArg::ChandleVal(value) => value.expressions(visit),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::ContainerGet { index, .. } => visit(index),
            Self::ContainerGetNested { indices, .. } => indices.iter_mut().for_each(visit),
            Self::Call { args, receiver, .. } => {
                if let Some(receiver) = receiver {
                    receiver.expressions_mut(visit);
                }
                for arg in args {
                    match arg {
                        IrCallArg::Val(value) => visit(value),
                        IrCallArg::ChandleVal(value) => value.expressions_mut(visit),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn object_type(
    model: &super::IrModel,
    index: usize,
    ty: IrObjectType,
) -> Result<(), super::IrValidationError> {
    if model
        .objects
        .get(index)
        .is_some_and(|object| object.ty == ty)
    {
        Ok(())
    } else {
        Err(super::IrValidationError::new(
            "object",
            format!("index {index} does not refer to {ty:?} storage"),
        ))
    }
}

fn pointer_object_type(
    model: &super::IrModel,
    index: usize,
) -> Result<(), super::IrValidationError> {
    if model
        .objects
        .get(index)
        .is_some_and(|object| matches!(object.ty, IrObjectType::Chandle | IrObjectType::Semaphore))
    {
        Ok(())
    } else {
        Err(super::IrValidationError::new(
            "object",
            format!("index {index} does not refer to pointer storage"),
        ))
    }
}
