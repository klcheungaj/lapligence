//! Oversized records use one descriptor column per recursive packed leaf type.
use super::*;
use crate::sim::ir::{IrFixedRecordValue, IrFixedValue};

fn record_columns(descriptor: &TypeDescriptor, path: &[String], dims: &[(i32, i32)], columns: &mut Vec<(Vec<String>, Vec<(i32, i32)>, TypeDescriptor)>) -> Result<(), String> {
    match &descriptor.shape {
        TypeShape::FixedArray { dimensions, element } => {
            let mut dims = dims.to_vec(); dims.extend(dimensions);
            record_columns(element, path, &dims, columns)
        }
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => {
            for member in &layout.members {
                let mut path = path.to_vec(); path.push(member.name.clone());
                record_columns(&member.descriptor, &path, dims, columns)?;
            }
            Ok(())
        }
        _ if Codegen::fixed_descriptor_width(descriptor).is_some() => {
            columns.push((path.to_vec(), dims.to_vec(), descriptor.clone())); Ok(())
        }
        _ => Err("descriptor record requires fixed integral leaves".into()),
    }
}

impl Codegen<'_> {
    pub(in super::super) fn collect_fixed_record(&mut self, path: &str, node: NodeId, activation: bool) -> Result<bool, String> {
        if self.fixed_records.contains_key(&node) { return Ok(true); }
        if !matches!(self.kind(node), NodeKind::Var { .. } | NodeKind::Array { .. } | NodeKind::FuncTask { .. } | NodeKind::FuncArg { .. }) { return Ok(false); }
        let Some(descriptor) = self.query_descriptor(node).cloned() else { return Ok(false); };
        if fixed_values::fixed_width_bits(&descriptor).is_none_or(|width| width <= u64::from(LLG_MAX_WIDTH)) { return Ok(false); }
        if matches!(&descriptor.shape, TypeShape::FixedArray { element, .. } if Self::fixed_descriptor_width(element).is_some()) { return Ok(false); }
        let mut columns = Vec::new(); record_columns(&descriptor, &[], &[], &mut columns)?;
        let mut leaves = Vec::new();
        for (member_path, mut dims, element) in columns {
            let scalar = dims.is_empty();
            dims.push((0, 0));
            let width = Self::fixed_descriptor_width(&element).ok_or("record leaf exceeds packed capacity")?;
            let total = fixed_values::fixed_array_cell_count(&dims)?;
            let ir = self.model.arrays.len();
            let name = format!("_llg_record_{ir}");
            let info = ArrayInfo { global: name.clone(), elem_width: width, signed: element.info.signed, real: false, shortreal: false, is_net: false, dims: dims.clone(), init: None, ir };
            self.model.arrays.push(IrArray { activation, descriptor: true, net_elements: Vec::new(), element_default: Self::fixed_descriptor_default(&element), c_name: name, hdl_name: if activation { String::new() } else { format!("{}.{}.{}", path, self.node(node).name, member_path.join(".")) }, elem_width: width, signed: element.info.signed, two_state: element.two_state, real: false, shortreal: false, dims, total });
            leaves.push(FixedRecordLeaf { path: member_path, array: info, scalar });
        }
        self.fixed_records.insert(node, leaves);
        Ok(true)
    }

    pub(in super::super) fn fixed_record_path(&self, node: NodeId) -> Option<(NodeId, Vec<String>, Vec<NodeId>)> {
        if self.fixed_records.contains_key(&node) { return Some((node, Vec::new(), Vec::new())); }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target: Some(target) }) => self.fixed_record_path(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, parts }) => {
                let (index, root) = refs.iter().enumerate().find_map(|(index, target)| target.filter(|target| self.fixed_records.contains_key(target)).map(|root| (index, root)))?;
                Some((root, parts[index + 1..].to_vec(), Vec::new()))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let (root, members, mut selected) = if let Some((root, members)) = self.db.array_select_path(node).filter(|(root, _)| self.fixed_records.contains_key(root)) { (root, members.to_vec(), Vec::new()) } else { self.fixed_record_path(*base)? };
                selected.extend(indices); Some((root, members, selected))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) if self.query_descriptor(*base).is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. })) => {
                let (root, members, mut selected) = self.fixed_record_path(*base)?;
                selected.push(*index); Some((root, members, selected))
            }
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.fixed_record_path(*operand),
            _ => None,
        }
    }

    pub(in super::super) fn fixed_record_scalar(&mut self, path: &str, node: NodeId) -> Result<Option<IrLhs>, String> {
        let Some((root, members, selected)) = self.fixed_record_path(node) else { return Ok(None); };
        let Some(leaf) = self.fixed_records[&root].iter().find(|leaf| leaf.path == members).cloned() else { return Ok(None); };
        if selected.len() + 1 != leaf.array.dims.len() { return Ok(None); }
        let mut indices = selected.into_iter().map(|index| self.lower_expr(path, index)).collect::<Result<Vec<_>, _>>()?;
        indices.push(lhs_integer_expr(0));
        Ok(Some(IrLhs::ArrayElem { arr: leaf.array.ir, indices, elem_sel: IrElemSel::Whole }))
    }

    pub(in super::super) fn fixed_record_views(&mut self, path: &str, node: NodeId) -> Result<Option<Vec<IrMemoryView>>, String> {
        let Some((root, members, selected)) = self.fixed_record_path(node) else { return Ok(None); };
        let leaves = self.fixed_records[&root].iter().filter(|leaf| leaf.path.starts_with(&members)).cloned().collect::<Vec<_>>();
        if leaves.is_empty() { return Ok(None); }
        let mut values = Vec::new();
        for index in &selected { values.push(self.lower_expr(path, *index)?); }
        let mut views = Vec::new();
        for leaf in leaves {
            if selected.len() >= leaf.array.dims.len() { return Err("record projection exceeds its stored dimensions".into()); }
            let dims = &leaf.array.dims;
            let strides = (0..dims.len()).map(|dimension| dims[dimension + 1..].iter().fold(1u64, |total, (left, right)| total * (u64::from(left.abs_diff(*right)) + 1))).collect::<Vec<_>>();
            let selectors = values.iter().enumerate().map(|(dimension, value)| crate::sim::ir::IrMemorySelector { dimension, left: dims[dimension].0, right: dims[dimension].1, stride: strides[dimension], value: value.clone() }).collect();
            let remaining = dims[selected.len()..].to_vec();
            let total = remaining.iter().fold(1u64, |total, (left, right)| total * (u64::from(left.abs_diff(*right)) + 1));
            views.push(IrMemoryView { array: leaf.array.ir, origin: 0, selectors, sliced: false, dims: remaining, strides: strides[selected.len()..].to_vec(), total });
        }
        Ok(Some(views))
    }

    pub(in super::super) fn lower_fixed_record_value(&mut self, path: &str, node: NodeId) -> Result<IrFixedRecordValue, String> {
        let node = self.p30_unwrap_cast(node);
        if let NodeKind::FuncCall { name, callee, .. } = self.kind(node) {
            let (name, callee) = (name.clone(), *callee);
            let (function, _) = self.resolve_callee_env(self.inst, &name, false, callee)?;
            if self.fixed_records.contains_key(&function) {
                let template = self.fixed_records[&function].clone();
                let mut arrays = Vec::new();
                for leaf in template {
                    let mut array = self.model.arrays[leaf.array.ir].clone();
                    array.activation = true; array.c_name = format!("_llg_record_{}", self.model.arrays.len()); array.hdl_name.clear();
                    arrays.push(self.model.arrays.len()); self.model.arrays.push(array);
                }
                let expression = self.lower_func_call_expr(path, node, &name, callee)?;
                let IrExprKind::CallFn(expression) = expression.kind else { return Err("record call requires typed operands".into()); };
                let mut args = expression.args;
                let output = self.model.funcs[expression.f].formals.iter().filter(|formal| formal.is_address()).count() - 1;
                let fields = arrays.iter().map(|array| IrFixedValue::Array(self.whole_fixed_view(*array))).collect();
                args.insert(output, IrCallArg::FixedRecord(Box::new(IrFixedRecordValue::Fields(fields))));
                return Ok(IrFixedRecordValue::Call { arrays, call: Box::new(IrCall::new(expression.f, args, expression.depth, Vec::new(), Vec::new())) });
            }
        }
        if let NodeKind::Expr(ExprKind::Operation { op: Operation::Conditional, operands, .. }) = self.kind(node) {
            let operands = operands.clone();
            let left = self.lower_fixed_record_value(path, operands[1])?;
            let right = self.lower_fixed_record_value(path, operands[2])?;
            let views = self.fixed_record_views(path, operands[1])?.ok_or("record conditional requires captured fields")?;
            let element_cells = views.iter().map(|view| if view.total == 1 { 0 } else { view.strides[0] }).collect();
            return Ok(IrFixedRecordValue::Conditional { selector: Box::new(self.lower_boolean_expr(path, operands[0])?), left: Box::new(left), right: Box::new(right), element_cells });
        }
        let views = self.fixed_record_views(path, node)?.ok_or("record value requires recursive descriptor storage")?;
        Ok(IrFixedRecordValue::Fields(views.into_iter().map(IrFixedValue::Array).collect()))
    }

    pub(in super::super) fn whole_fixed_view(&self, array: usize) -> IrMemoryView {
        let array_info = &self.model.arrays[array];
        let dims = array_info.dims.clone();
        let strides = (0..dims.len()).map(|dimension| dims[dimension + 1..].iter().fold(1u64, |total, (left, right)| total * (u64::from(left.abs_diff(*right)) + 1))).collect();
        IrMemoryView { array, origin: 0, selectors: Vec::new(), sliced: false, dims, strides, total: array_info.total }
    }
}
