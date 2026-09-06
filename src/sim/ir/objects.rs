//! Non-integral storage and expressions, kept distinct from packed vectors.

use super::IrExpr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Non-integral scalar storage categories, independent of the packed backend.
pub enum IrObjectType {
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
    Read(usize),
    LocalRead(String),
    Call {
        function: usize,
        args: Vec<IrExpr>,
        depth: super::IrDepth,
    },
    Concat(Vec<IrStringExpr>),
    Repeat(Box<IrStringExpr>, Box<IrExpr>),
    FromPacked(Box<IrExpr>),
    Case(Box<IrStringExpr>, bool),
    Substr(Box<IrStringExpr>, Box<IrExpr>, Box<IrExpr>),
}

#[derive(Clone, Debug, PartialEq)]
/// Native-pointer values; arithmetic and packed conversion are not represented.
pub enum IrChandleExpr {
    Null,
    Read(usize),
    LocalRead(String),
    FormalRead(usize),
    Call {
        function: usize,
        args: Vec<IrChandleExpr>,
        depth: super::IrDepth,
    },
}

/// Queries produce packed values, never an integer encoding of an object.
#[derive(Clone, Debug, PartialEq)]
pub enum IrObjectQuery {
    StringLen(IrStringExpr),
    StringGetc(IrStringExpr, Box<IrExpr>),
    StringCompare(IrStringExpr, IrStringExpr, bool),
    StringAtoi(IrStringExpr, u32),
    StringPacked(IrStringExpr),
    ChandleEq(IrChandleExpr, IrChandleExpr),
}

#[derive(Clone, Debug, PartialEq)]
/// Object mutations and output. String values are copied, not storage aliases.
pub enum IrObjectStmt {
    StringPrint(IrStringExpr),
    StringAssign(usize, IrStringExpr),
    StringAssignLocal(String, IrStringExpr),
    StringPutc(usize, IrExpr, IrExpr),
    StringItoa(usize, IrExpr, u32),
    ChandleAssign(usize, IrChandleExpr),
    ChandleAssignLocal(String, IrChandleExpr),
}

impl IrStringExpr {
    pub(in crate::sim) fn validate(
        &self,
        model: &super::IrModel,
        string_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        let mut valid = true;
        self.expressions(&mut |expr| valid &= !expr.is_real());
        if !valid {
            return Err(super::IrValidationError::new(
                "string",
                "string operation requires packed integral operands",
            ));
        }
        match self {
            Self::LocalRead(name) if name == "_ret" && string_return == Some(true) => Ok(()),
            Self::LocalRead(_) => Err(super::IrValidationError::new(
                "string local",
                "unknown string return storage",
            )),
            Self::Call {
                function,
                args,
                depth,
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
            Self::Read(index) => object_type(model, *index, IrObjectType::String),
            Self::Concat(parts) => parts
                .iter()
                .try_for_each(|part| part.validate(model, string_return)),
            Self::Repeat(value, _) | Self::Case(value, _) | Self::Substr(value, _, _) => {
                value.validate(model, string_return)
            }
            _ => Ok(()),
        }
    }
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Literal(_) | Self::Read(_) | Self::LocalRead(_) => {}
            Self::Call { args, .. } => args.iter().for_each(visit),
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
            Self::Literal(_) | Self::Read(_) | Self::LocalRead(_) => {}
            Self::Call { args, .. } => args.iter_mut().for_each(visit),
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
            Self::Case(value, _) => value.expressions_mut(visit),
            Self::Substr(value, first, last) => {
                value.expressions_mut(visit);
                visit(first);
                visit(last);
            }
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
            | Self::StringPacked(value) => value.validate(model, string_return),
            Self::StringCompare(a, b, _) => {
                a.validate(model, string_return)?;
                b.validate(model, string_return)
            }
            Self::ChandleEq(a, b) => {
                a.validate(model, formals, chandle_return)?;
                b.validate(model, formals, chandle_return)
            }
        }
    }
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::StringLen(value) | Self::StringAtoi(value, _) | Self::StringPacked(value) => {
                value.expressions(visit)
            }
            Self::StringGetc(value, index) => {
                value.expressions(visit);
                visit(index);
            }
            Self::StringCompare(a, b, _) => {
                a.expressions(visit);
                b.expressions(visit);
            }
            Self::ChandleEq(..) => {}
        }
    }
    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::StringLen(value) | Self::StringAtoi(value, _) | Self::StringPacked(value) => {
                value.expressions_mut(visit)
            }
            Self::StringGetc(value, index) => {
                value.expressions_mut(visit);
                visit(index);
            }
            Self::StringCompare(a, b, _) => {
                a.expressions_mut(visit);
                b.expressions_mut(visit);
            }
            Self::ChandleEq(..) => {}
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
                if name != "_ret" || string_return != Some(true) {
                    return Err(super::IrValidationError::new(
                        "string local",
                        "unknown string return storage",
                    ));
                }
                value.validate(model, string_return)
            }
            Self::StringPutc(index, _, _) | Self::StringItoa(index, _, _) => {
                object_type(model, *index, IrObjectType::String)
            }
            Self::ChandleAssign(index, value) => {
                object_type(model, *index, IrObjectType::Chandle)?;
                value.validate(model, formals, chandle_return)
            }
            Self::ChandleAssignLocal(name, value) => {
                if name != "_ret" || chandle_return != Some(true) {
                    return Err(super::IrValidationError::new(
                        "chandle local",
                        "unknown chandle return storage",
                    ));
                }
                value.validate(model, formals, chandle_return)
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
            Self::StringItoa(_, value, _) => visit(value),
            Self::ChandleAssign(..) | Self::ChandleAssignLocal(..) => {}
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
            Self::StringItoa(_, value, _) => visit(value),
            Self::ChandleAssign(..) | Self::ChandleAssignLocal(..) => {}
        }
    }
}

impl IrChandleExpr {
    fn validate(
        &self,
        model: &super::IrModel,
        formals: &[super::IrFormal],
        chandle_return: Option<bool>,
    ) -> Result<(), super::IrValidationError> {
        match self {
            Self::Null => Ok(()),
            Self::LocalRead(name) if name == "_ret" && chandle_return == Some(true) => Ok(()),
            Self::LocalRead(_) => Err(super::IrValidationError::new(
                "chandle local",
                "unknown chandle return storage",
            )),
            Self::FormalRead(index) => {
                if formals
                    .get(*index)
                    .is_some_and(|formal| formal.chandle && !formal.is_out)
                {
                    Ok(())
                } else {
                    Err(super::IrValidationError::new(
                        "chandle formal",
                        "index does not refer to a chandle input formal",
                    ))
                }
            }
            Self::Read(index) => object_type(model, *index, IrObjectType::Chandle),
            Self::Call {
                function,
                args,
                depth,
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
                if !callee.ret_chandle
                    || callee.formals.len() != args.len()
                    || callee
                        .formals
                        .iter()
                        .any(|formal| formal.is_out || !formal.chandle)
                {
                    return Err(super::IrValidationError::new(
                        "chandle call",
                        "callee must return chandle and have only chandle input formals",
                    ));
                }
                args.iter()
                    .try_for_each(|arg| arg.validate(model, formals, chandle_return))
            }
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
