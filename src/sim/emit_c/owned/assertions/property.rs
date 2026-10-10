//! Property programs: one node table per assertion, its sequence graphs and
//! one sampled atom callback (IEEE 1800-2009 16.12-16.13).
use super::*;
use crate::sim::ir::{IrProperty, IrPropertyBinaryOp, IrPropertyNode};

pub(super) fn program_name(index: usize) -> String {
    format!("llg_assertion_{index}_property")
}

fn atom_callback(
    model: &IrModel,
    constants: &super::super::super::constants::PackedConstants,
    backend: crate::sim::value_backend::ValueBackend,
    name: &str,
    property: &IrProperty,
) -> Result<String, String> {
    let ctx = RCtx {
        value_backend: backend,
        model,
        func: None,
        sampled: true,
        activation_label: None,
        constants: Some(constants),
    };
    let mut frame = callback_frame(&ctx);
    frame.line("switch (atom) {");
    for (atom_index, atom) in property.atoms().iter().enumerate() {
        frame.line(format!("case {atom_index}u: {{"));
        let value = frame.expression(atom)?;
        if value.width == 0 {
            return Err("property atom must be packed".to_owned());
        }
        let result = frame.scalar("int", value.truth());
        frame.discard(value);
        frame.line("llg_value_scopes_end_since(_llg_frame_base);");
        frame.line(format!("return {result}; }}"));
    }
    frame.line("default: break; }");
    frame.line("llg_value_scopes_end_since(_llg_frame_base);");
    frame.line("return 0;");
    Ok(format!(
        "static int {name}(uint32_t atom, void* data) {{\n{}{}\n}}\n\n",
        frame.prologue(),
        frame.body()
    ))
}

fn bound(max: Option<u32>) -> String {
    max.map(|max| format!("{max}ULL"))
        .unwrap_or_else(|| "LLG_SEQUENCE_UNBOUNDED".to_owned())
}

/// `{kind, flags, first, second, third, min, max}` for one node.
fn node_row(node: &IrPropertyNode) -> String {
    const NONE: &str = "LLG_PROPERTY_NONE";
    fn flag(set: bool, name: &'static str) -> &'static str {
        if set {
            name
        } else {
            "0"
        }
    }
    let (kind, flags, first, second, third, min, max) = match *node {
        IrPropertyNode::Boolean { atom } => (
            "LLG_PROPERTY_BOOLEAN",
            "0".to_owned(),
            format!("{atom}u"),
            NONE.to_owned(),
            NONE.to_owned(),
            0,
            "0ULL".to_owned(),
        ),
        IrPropertyNode::Sequence { sequence, strong } => (
            "LLG_PROPERTY_SEQUENCE",
            flag(strong, "LLG_PROPERTY_STRONG").to_owned(),
            format!("{sequence}u"),
            NONE.to_owned(),
            NONE.to_owned(),
            0,
            "0ULL".to_owned(),
        ),
        IrPropertyNode::Not { operand } => (
            "LLG_PROPERTY_NOT",
            "0".to_owned(),
            format!("{operand}u"),
            NONE.to_owned(),
            NONE.to_owned(),
            0,
            "0ULL".to_owned(),
        ),
        IrPropertyNode::Binary { op, left, right } => (
            match op {
                IrPropertyBinaryOp::And => "LLG_PROPERTY_AND",
                IrPropertyBinaryOp::Or => "LLG_PROPERTY_OR",
                IrPropertyBinaryOp::Implies => "LLG_PROPERTY_IMPLIES",
                IrPropertyBinaryOp::Iff => "LLG_PROPERTY_IFF",
            },
            "0".to_owned(),
            format!("{left}u"),
            format!("{right}u"),
            NONE.to_owned(),
            0,
            "0ULL".to_owned(),
        ),
        IrPropertyNode::Implication {
            antecedent,
            consequent,
            overlapped,
            followed_by,
        } => (
            if followed_by {
                "LLG_PROPERTY_FOLLOWED_BY"
            } else {
                "LLG_PROPERTY_IMPLICATION"
            },
            flag(overlapped, "LLG_PROPERTY_OVERLAPPED").to_owned(),
            format!("{antecedent}u"),
            format!("{consequent}u"),
            NONE.to_owned(),
            0,
            "0ULL".to_owned(),
        ),
        IrPropertyNode::If {
            condition,
            then,
            otherwise,
        } => (
            "LLG_PROPERTY_IF",
            "0".to_owned(),
            format!("{condition}u"),
            format!("{then}u"),
            otherwise
                .map(|node| format!("{node}u"))
                .unwrap_or_else(|| NONE.to_owned()),
            0,
            "0ULL".to_owned(),
        ),
        IrPropertyNode::Nexttime {
            count,
            strong,
            operand,
        } => (
            "LLG_PROPERTY_NEXTTIME",
            flag(strong, "LLG_PROPERTY_STRONG").to_owned(),
            format!("{operand}u"),
            NONE.to_owned(),
            NONE.to_owned(),
            count,
            format!("{count}ULL"),
        ),
        IrPropertyNode::Always {
            min,
            max,
            strong,
            operand,
        } => (
            "LLG_PROPERTY_ALWAYS",
            flag(strong, "LLG_PROPERTY_STRONG").to_owned(),
            format!("{operand}u"),
            NONE.to_owned(),
            NONE.to_owned(),
            min,
            bound(max),
        ),
        IrPropertyNode::Eventually {
            min,
            max,
            strong,
            operand,
        } => (
            "LLG_PROPERTY_EVENTUALLY",
            flag(strong, "LLG_PROPERTY_STRONG").to_owned(),
            format!("{operand}u"),
            NONE.to_owned(),
            NONE.to_owned(),
            min,
            bound(max),
        ),
        IrPropertyNode::Until {
            left,
            right,
            strong,
            overlapping,
        } => (
            "LLG_PROPERTY_UNTIL",
            format!(
                "{} | {}",
                flag(strong, "LLG_PROPERTY_STRONG"),
                flag(overlapping, "LLG_PROPERTY_OVERLAPPED")
            ),
            format!("{left}u"),
            format!("{right}u"),
            NONE.to_owned(),
            0,
            "0ULL".to_owned(),
        ),
        IrPropertyNode::Abort {
            condition,
            accept,
            sync,
            operand,
        } => (
            "LLG_PROPERTY_ABORT",
            format!(
                "{} | {}",
                flag(accept, "LLG_PROPERTY_ACCEPT"),
                flag(sync, "LLG_PROPERTY_SYNC")
            ),
            format!("{condition}u"),
            format!("{operand}u"),
            NONE.to_owned(),
            0,
            "0ULL".to_owned(),
        ),
    };
    format!("    {{{kind}, {flags}, {first}, {second}, {third}, {min}ULL, {max}}},\n")
}

pub(super) fn render(
    model: &IrModel,
    constants: &super::super::super::constants::PackedConstants,
    backend: crate::sim::value_backend::ValueBackend,
    index: usize,
    property: &IrProperty,
) -> Result<String, String> {
    let mut out = String::new();
    let mut graphs = Vec::new();
    for (sequence_index, sequence) in property.sequences().iter().enumerate() {
        let role = format!("prop{sequence_index}");
        out.push_str(&sequence::render(
            model, constants, backend, index, &role, sequence,
        )?);
        graphs.push(format!("&{}", assertion_sequence_name(index, &role)));
    }
    let name = program_name(index);
    let atom_name = format!("{name}_atom");
    let atom = if property.atoms().is_empty() {
        "NULL".to_owned()
    } else {
        out.push_str(&atom_callback(
            model, constants, backend, &atom_name, property,
        )?);
        atom_name
    };
    let graphs_name = format!("{name}_sequences");
    let graphs_ptr = if graphs.is_empty() {
        "NULL".to_owned()
    } else {
        out.push_str(&format!(
            "static const llg_sequence_graph_t* const {graphs_name}[{}] = {{ {} }};\n",
            graphs.len(),
            graphs.join(", ")
        ));
        graphs_name
    };
    let nodes_name = format!("{name}_nodes");
    out.push_str(&format!(
        "static const llg_property_node_t {nodes_name}[{}] = {{\n",
        property.nodes().len()
    ));
    for node in property.nodes() {
        out.push_str(&node_row(node));
    }
    out.push_str("};\n");
    out.push_str(&format!(
        "static const llg_property_program_t {name} = {{ {}u, {nodes_name}, {}u, {}u, {graphs_ptr}, {}u, {atom} }};\n\n",
        property.nodes().len(),
        property.root(),
        property.sequences().len(),
        property.atoms().len(),
    ));
    Ok(out)
}
