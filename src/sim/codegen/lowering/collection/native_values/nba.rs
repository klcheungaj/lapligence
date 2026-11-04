//! Nonblocking writes inside native record values (SV 10.4.2).
//!
//! A leaf of a native value moves when the whole value is replaced, so a
//! queued write cannot keep its address. The source is captured at issue and
//! the runtime finds the item again at commit, from a locator request
//! (`IrContainerStmt::NbaLocate`) that fixes the element position.
use super::*;
use crate::sim::ir::{IrValueItemRoot, IrValueSlot};

/// The typed destination an element member NBA names at issue.
enum ElementNbaTarget {
    Object(String),
    Packed(IrLhs),
}

/// A queued write of `value` to the located leaf whose access is `name`.
fn element_leaf_nba(
    model: &IrModel,
    name: String,
    ty: IrClassFieldType,
    value: LeafValue,
    ticks: IrDelay,
) -> Result<IrStmt, String> {
    Ok(match (ty, value) {
        (IrClassFieldType::String, LeafValue::String(rhs)) => IrStmt::DelayedStringAssign {
            target: name,
            rhs,
            ticks,
        },
        (IrClassFieldType::Chandle, LeafValue::Chandle(rhs)) => IrStmt::DelayedChandleAssign {
            target: name,
            rhs,
            ticks,
        },
        (
            IrClassFieldType::Packed {
                width,
                signed,
                two_state,
            },
            LeafValue::Packed(value),
        ) => {
            let lhs = IrLhs::WholeRef {
                addr: format!("&{name}"),
                width,
                signed,
                two_state,
                shortreal: false,
            };
            let rhs = apply_lhs_assignment_context(model, &lhs, value);
            IrStmt::DelayedAssign { lhs, rhs, ticks }
        }
        (IrClassFieldType::Real { shortreal }, LeafValue::Real(value)) => {
            let lhs = IrLhs::WholeRef {
                addr: format!("&{name}"),
                width: 0,
                signed: false,
                two_state: false,
                shortreal,
            };
            let rhs = apply_lhs_assignment_context(model, &lhs, value);
            IrStmt::DelayedAssign { lhs, rhs, ticks }
        }
        _ => return Err("element member write pairs different leaf kinds".to_owned()),
    })
}

impl Codegen<'_> {
    /// `a[i].m <= v`, also a select inside a packed member: one leaf of an
    /// element of persistent fixed-array storage. The element position and
    /// the value are captured here; the write finds the item when it commits
    /// because a whole-element write may replace the value tree before the
    /// NBA region (SV 10.4.2). Whole sub-records and member arrays keep an
    /// explicit diagnostic.
    pub(in crate::sim::codegen) fn lower_element_member_nba(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        delay: Option<IrDelay>,
    ) -> Result<IrStmt, String> {
        let Some((selection, leaf)) = self.element_nba_leaf(lhs)? else {
            return self.lower_element_subrecord_nba(path, lhs, rhs, delay);
        };
        self.require_persistent_view(path, selection.container)?;
        let mut statements = Vec::new();
        let slot = self.element_nba_slot(path, &selection, &mut statements)?;
        let IrValueSlot::Element { indices, .. } = &slot else {
            unreachable!("element selections lower to element slots");
        };
        let root = IrValueItemRoot::Element(IrChandleExpr::ContainerElement {
            container: selection.container,
            indices: indices.clone(),
            key: None,
            write: false,
        });
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::NbaLocate {
            root,
            items: leaf.items.clone(),
        })));
        let ticks = delay.unwrap_or(IrDelay::Constant(0));
        self.element_nba_target = Some(slot);
        let target = match leaf.ty {
            IrClassFieldType::String | IrClassFieldType::Chandle => self
                .element_leaf_symbol(path, lhs, false)
                .and_then(|symbol| {
                    symbol
                        .map(|(name, _)| ElementNbaTarget::Object(name))
                        .ok_or_else(|| "element member has no storage".to_owned())
                }),
            _ => self.lower_lhs(path, lhs).map(ElementNbaTarget::Packed),
        };
        self.element_nba_target = None;
        statements.push(match target? {
            ElementNbaTarget::Object(target) => match leaf.ty {
                IrClassFieldType::String => IrStmt::DelayedStringAssign {
                    target,
                    rhs: self.lower_string(path, rhs)?,
                    ticks,
                },
                _ => IrStmt::DelayedChandleAssign {
                    target,
                    rhs: self.lower_chandle(path, rhs)?,
                    ticks,
                },
            },
            ElementNbaTarget::Packed(lhs) => {
                let value = self.lower_expr(path, rhs)?;
                let rhs = apply_lhs_assignment_context(&self.model, &lhs, value);
                IrStmt::DelayedAssign { lhs, rhs, ticks }
            }
        });
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::NbaLocateEnd)));
        Ok(IrStmt::Block(statements))
    }

    /// `a[i].inner <= v`, `a[i].arr <= v`: the source is captured into a
    /// temporary of the element type at issue, then every scalar leaf below
    /// the selection queues its own located write.
    fn lower_element_subrecord_nba(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        delay: Option<IrDelay>,
    ) -> Result<IrStmt, String> {
        let (selection, prefix, leaves, containers) = self.element_member_leaves(path, lhs)?;
        self.require_persistent_view(path, selection.container)?;
        let mut statements = Vec::new();
        let (temporary, descriptor) =
            self.element_nba_temporary(selection.clone(), lhs, &prefix)?;
        statements.push(IrStmt::NativeValueDeclare(temporary));
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix: prefix.clone(),
        };
        if containers.len() == 1 && containers[0].path == prefix {
            // A whole container member: any container source fills its copy.
            let layout = self.native_layout_of_value(temporary)?;
            let position = layout
                .containers
                .iter()
                .position(|candidate| candidate.path == prefix)
                .ok_or("element temporary has no such container")?;
            let companion = self.model.native_values[temporary].companions[position];
            statements.push(self.lower_container_into(path, lhs, companion, rhs)?);
        } else {
            statements.push(self.native_assign_into(path, &endpoint, &descriptor, rhs, false)?);
        }
        let slot = self.element_nba_slot(path, &selection, &mut statements)?;
        let IrValueSlot::Element { indices, .. } = &slot else {
            unreachable!("element selections lower to element slots");
        };
        let indices = indices.clone();
        let ticks = delay.unwrap_or(IrDelay::Constant(0));
        let source = self.native_layout_of_value(temporary)?.leaves.clone();
        for leaf in leaves {
            let Some(value) = source.iter().find(|candidate| candidate.path == leaf.path) else {
                return Err("element temporary has no such leaf".to_owned());
            };
            let value = self.native_leaf_read(temporary, value);
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::NbaLocate {
                root: IrValueItemRoot::Element(IrChandleExpr::ContainerElement {
                    container: selection.container,
                    indices: indices.clone(),
                    key: None,
                    write: false,
                }),
                items: leaf.items.clone(),
            })));
            let name = self.element_nba_access(path, &selection, &slot, &leaf)?;
            statements.push(element_leaf_nba(
                &self.model,
                name,
                leaf.ty,
                value,
                ticks.clone(),
            )?);
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::NbaLocateEnd)));
        }
        // Container members travel in the temporary's companions.
        let layout = self.native_layout_of_value(temporary)?.containers.clone();
        let companions = self.model.native_values[temporary].companions.clone();
        for leaf in containers {
            let source = layout
                .iter()
                .position(|candidate| candidate.path == leaf.path)
                .and_then(|position| companions.get(position).copied())
                .ok_or("element temporary has no such container")?;
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::NbaLocate {
                root: IrValueItemRoot::Element(IrChandleExpr::ContainerElement {
                    container: selection.container,
                    indices: indices.clone(),
                    key: None,
                    write: false,
                }),
                items: leaf.items.clone(),
            })));
            statements.push(IrStmt::Container(Box::new(
                IrContainerStmt::ContainerNonblocking {
                    target: None,
                    src: source,
                    delay: Some(ticks.clone()),
                },
            )));
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::NbaLocateEnd)));
        }
        Ok(IrStmt::Block(statements))
    }
}
