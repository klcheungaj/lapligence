//! Enumerations.

use super::*;

impl Codegen<'_> {
    /// Return the owned metadata for an expression whose resolved type is an
    /// enum. This uses only the captured descriptor/type tables.
    pub(in super::super) fn enum_metadata_for_expr(
        &self,
        node: NodeId,
    ) -> Option<&crate::core::db::EnumTypeMetadata> {
        let descriptor = self.query_descriptor(node)?;
        self.db.enum_type_metadata(descriptor.id)
    }

    fn enum_members(
        metadata: &crate::core::db::EnumTypeMetadata,
    ) -> Result<Vec<IrEnumMember>, String> {
        metadata
            .members
            .iter()
            .map(|member| {
                let Val::Bits(value) = &member.value else {
                    return Err(format!(
                        "enum member {} has a non-integral value",
                        member.name
                    ));
                };
                let value = val_to_const(value)?;
                if value.width != metadata.width || value.signed != metadata.signed {
                    return Err(format!(
                        "enum member {} value type does not match its enum type",
                        member.name
                    ));
                }
                Ok(IrEnumMember {
                    value: IrExpr::new(
                        IrExprKind::Const(value),
                        metadata.width,
                        metadata.signed,
                        None,
                    ),
                    name: member.name.as_bytes().to_vec(),
                })
            })
            .collect()
    }

    fn enum_default(metadata: &crate::core::db::EnumTypeMetadata) -> Result<IrExpr, String> {
        let limbs = metadata.width.div_ceil(64) as usize;
        let mut x = vec![0; limbs];
        if !metadata.two_state {
            x.fill(u64::MAX);
            if let Some(last) = x.last_mut() {
                if !metadata.width.is_multiple_of(64) {
                    *last = (1_u64 << (metadata.width % 64)) - 1;
                }
            }
        }
        let value = IrConst::packed(
            vec![0; limbs],
            x,
            vec![0; limbs],
            metadata.width,
            metadata.signed,
            None,
        )
        .map_err(|error| error.to_string())?;
        Ok(IrExpr::new(
            IrExprKind::Const(value),
            metadata.width,
            metadata.signed,
            None,
        ))
    }

    fn enum_step_default() -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(
                IrConst::packed(vec![1], vec![0], vec![0], 32, false, None)
                    .expect("fixed-width enum step literal is valid"),
            ),
            32,
            false,
            None,
        )
    }

    /// Lower the numeric enum methods. Type-only methods deliberately leave
    /// their receiver unevaluated, while navigation evaluates it once.
    pub(in super::super) fn lower_enum_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.as_str(), *receiver),
            _ => return Ok(None),
        };
        let method = match name {
            "first" => IrEnumMethod::First,
            "last" => IrEnumMethod::Last,
            "next" => IrEnumMethod::Next,
            "prev" => IrEnumMethod::Prev,
            "num" => IrEnumMethod::Num,
            _ => return Ok(None),
        };
        let metadata = self
            .enum_metadata_for_expr(receiver)
            .cloned()
            .ok_or_else(|| format!("enum method {name} has no owned enum metadata in {path}"))?;
        let args = self.node(node).children.get(1..).unwrap_or_default();
        if !matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev) && !args.is_empty() {
            return Err(format!("enum method {name} takes no arguments in {path}"));
        }
        if matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev) && args.len() > 1 {
            return Err(format!(
                "enum method {name} takes at most one argument in {path}"
            ));
        }
        let members = Self::enum_members(&metadata)?;
        if members.is_empty() {
            return Err(format!(
                "enum method {name} has no declared members in {path}"
            ));
        }
        let default = Self::enum_default(&metadata)?;
        let receiver = matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev)
            .then(|| self.lower_expr(path, receiver))
            .transpose()?
            .map(Box::new);
        let step = if matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev) {
            Some(Box::new(match args {
                [] => Self::enum_step_default(),
                [arg] => ir_to_storage(self.lower_expr(path, *arg)?, 32, false, true)?,
                _ => unreachable!(),
            }))
        } else {
            None
        };
        let (width, signed) = if method == IrEnumMethod::Num {
            (32, true)
        } else {
            (metadata.width, metadata.signed)
        };
        Ok(Some(IrExpr::new(
            IrExprKind::EnumMethod(Box::new(IrEnumQuery {
                method,
                receiver,
                step,
                members,
                default,
            })),
            width,
            signed,
            None,
        )))
    }

    /// Lower name to an owned string expression with declaration names
    /// captured for the receiver's enum type.
    pub(super) fn lower_enum_name(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.as_str(), *receiver),
            _ => return Ok(None),
        };
        if name != "name" {
            return Ok(None);
        }
        let args = self.node(node).children.get(1..).unwrap_or_default();
        if !args.is_empty() {
            return Err(format!("enum method name takes no arguments in {path}"));
        }
        let metadata = self
            .enum_metadata_for_expr(receiver)
            .cloned()
            .ok_or_else(|| format!("enum method name has no owned enum metadata in {path}"))?;
        let members = Self::enum_members(&metadata)?;
        if members.is_empty() {
            return Err(format!(
                "enum method name has no declared members in {path}"
            ));
        }
        Ok(Some(IrStringExpr::EnumName {
            receiver: Box::new(self.lower_expr(path, receiver)?),
            members,
        }))
    }
}
