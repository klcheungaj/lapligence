//! Owned source module graph derived from Slang semantic records.

use super::*;

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ModuleGraph {
    pub definitions: Vec<ModuleGraphDefinition>,
    pub elaborated_types: Vec<ModuleGraphElaboratedType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModuleGraphPackedRange {
    pub left: i128,
    pub right: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleGraphElaboratedType {
    pub instance: String,
    pub name: String,
    pub packed_ranges: Vec<Option<ModuleGraphPackedRange>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphDefinition {
    pub id: String,
    pub name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub ports: Vec<ModuleGraphPort>,
    pub params: Vec<ModuleGraphParameter>,
    pub signals: Vec<ModuleGraphSignal>,
    pub children: Vec<ModuleGraphInstance>,
    pub generated_scopes: Vec<ModuleGraphGenerateScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleGraphLocation {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ModuleGraphTypeShape {
    pub packed_dimensions: usize,
    pub unpacked_dimensions: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphPort {
    pub name: String,
    pub direction: Direction,
    pub ty: TypeInfo,
    pub detail: Option<String>,
    pub location: Option<ModuleGraphLocation>,
    pub display_type: Option<String>,
    pub display_shape: ModuleGraphTypeShape,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphSignal {
    pub name: String,
    pub kind: String,
    pub ty: TypeInfo,
    pub detail: Option<String>,
    pub location: Option<ModuleGraphLocation>,
    pub display_type: Option<String>,
    pub display_shape: ModuleGraphTypeShape,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphParameter {
    pub name: String,
    pub ty: TypeInfo,
    pub local: bool,
    pub detail: Option<String>,
    pub location: Option<ModuleGraphLocation>,
    pub display_type: Option<String>,
    pub display_shape: ModuleGraphTypeShape,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphInstance {
    pub name: String,
    pub module_type: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleGraphGenerateScope {
    pub name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub children: Vec<ModuleGraphInstance>,
    pub nested: Vec<ModuleGraphGenerateScope>,
}

pub(crate) fn module_graph_definition_id(
    name: &str,
    file: Option<&str>,
    line: u32,
    col: u32,
) -> String {
    format!(
        "{}|{}|{line}|{col}",
        clean_name(name),
        file.unwrap_or("<unknown>")
    )
}

pub(super) fn module_graph_from_slang(
    snapshot: &llg::ffi::slang::Snapshot,
    sources: &[(&str, &str)],
    database: Option<&llg::core::db::Db>,
) -> ModuleGraph {
    use llg::ffi::slang::{SemanticDefinitionKind, SemanticKind};
    let files: HashMap<_, _> = snapshot
        .files
        .iter()
        .map(|file| (file.id, file.name.as_str()))
        .collect();
    let texts: HashMap<_, _> = sources.iter().copied().collect();
    let positions: HashMap<_, _> = sources
        .iter()
        .map(|&(file, text)| (file, tokens::SourcePositions::new(text)))
        .collect();
    let declaration_types: HashMap<_, _> = database
        .into_iter()
        .flat_map(llg::core::db::Db::nodes)
        .filter_map(|node| {
            let ty = match node.kind() {
                llg::core::db::NodeKind::Port { ty, .. }
                | llg::core::db::NodeKind::Net { ty, .. }
                | llg::core::db::NodeKind::Var { ty }
                | llg::core::db::NodeKind::Genvar { ty }
                | llg::core::db::NodeKind::Array { ty }
                | llg::core::db::NodeKind::Param { ty, .. } => ty,
                _ => return None,
            };
            Some((
                (
                    node.file()?.to_owned(),
                    node.line(),
                    node.column(),
                    node.name().to_owned(),
                ),
                ty.clone(),
            ))
        })
        .collect();
    let mut definitions = Vec::new();
    let mut definition_keys = HashMap::new();
    for node in &snapshot.semantic_nodes {
        if node.kind != SemanticKind::Definition
            || node.definition_kind != Some(SemanticDefinitionKind::Module)
            || node.name.is_empty()
        {
            continue;
        }
        let (file, line, col, end_line, end_col) = node
            .range
            .and_then(|range| {
                let file = *files.get(&range.file_id)?;
                let positions = positions.get(file)?;
                let (line, col) = positions.position(range.start);
                let (end_line, end_col) = positions.position(range.end);
                Some((Some(file.to_owned()), line, col, end_line, end_col))
            })
            .unwrap_or((None, 0, 0, 0, 0));
        let id = module_graph_definition_id(&node.name, file.as_deref(), line, col);
        definition_keys.insert(node.id, id.clone());
        definitions.push(ModuleGraphDefinition {
            id,
            name: node.name.clone(),
            file,
            line,
            col,
            end_line,
            end_col,
            ports: Vec::new(),
            params: Vec::new(),
            signals: Vec::new(),
            children: Vec::new(),
            generated_scopes: Vec::new(),
        });
    }
    definitions.sort_by(|left, right| {
        (&left.name, &left.file, left.line, left.col).cmp(&(
            &right.name,
            &right.file,
            right.line,
            right.col,
        ))
    });
    definitions.dedup_by(|left, right| left.id == right.id);

    let semantic_by_id: HashMap<_, _> = snapshot
        .semantic_nodes
        .iter()
        .map(|node| (node.id, node))
        .collect();
    if let Some(database) = database {
        fn direct_instance_children(
            database: &llg::core::db::Db,
            parent: llg::core::db::NodeId,
            result: &mut Vec<llg::core::db::NodeId>,
        ) {
            let mut pending = database.node(parent).children().to_vec();
            while let Some(child) = pending.pop() {
                if matches!(
                    database.node(child).kind(),
                    llg::core::db::NodeKind::ModuleInst { .. }
                ) {
                    result.push(child);
                } else if matches!(
                    database.node(child).kind(),
                    llg::core::db::NodeKind::GenScope | llg::core::db::NodeKind::GenScopeArray
                ) {
                    pending.extend_from_slice(database.node(child).children());
                }
            }
        }

        let mut owners = database
            .node_ids()
            .filter(|id| {
                matches!(
                    database.node(*id).kind(),
                    llg::core::db::NodeKind::ModuleInst { is_top: true, .. }
                )
            })
            .collect::<Vec<_>>();
        while let Some(owner_id) = owners.pop() {
            let owner = database.node(owner_id);
            let llg::core::db::NodeKind::ModuleInst {
                def_name: owner_definition,
                ..
            } = owner.kind()
            else {
                continue;
            };
            let mut children = Vec::new();
            direct_instance_children(database, owner_id, &mut children);
            for child_id in children {
                let child_node = database.node(child_id);
                let llg::core::db::NodeKind::ModuleInst { def_name, .. } = child_node.kind() else {
                    continue;
                };
                let child = ModuleGraphInstance {
                    name: child_node.name().to_owned(),
                    module_type: clean_name(def_name).to_owned(),
                    file: child_node.file().map(str::to_owned),
                    line: child_node.line(),
                    col: child_node.column(),
                };
                for definition in definitions.iter_mut().filter(|definition| {
                    clean_name(&definition.name) == clean_name(owner_definition)
                }) {
                    if !definition.children.contains(&child) {
                        definition.children.push(child.clone());
                    }
                }
                owners.push(child_id);
            }
        }
    }
    for node in &snapshot.semantic_nodes {
        if node.kind != SemanticKind::Instance
            || node.is_top
            || (database.is_some() && !node.is_uninstantiated)
            || node.name.is_empty()
            || node.definition_name.is_empty()
        {
            continue;
        }
        let mut owner = node
            .parent_id
            .and_then(|id| semantic_by_id.get(&id).copied());
        while owner.is_some_and(|parent| {
            !matches!(
                parent.kind,
                SemanticKind::Instance | SemanticKind::Definition
            )
        }) {
            owner = owner
                .and_then(|parent| parent.parent_id)
                .and_then(|id| semantic_by_id.get(&id).copied());
        }
        let Some(owner) = owner else { continue };
        if owner.range == node.range
            && owner.name == node.name
            && owner.definition_name == node.definition_name
        {
            continue;
        }
        let owner_id = if owner.kind == SemanticKind::Definition {
            Some(owner.id)
        } else {
            owner.target_id
        };
        let Some(owner_key) = owner_id.and_then(|id| definition_keys.get(&id)) else {
            continue;
        };
        let (file, line, col) = node
            .range
            .and_then(|range| {
                let file = *files.get(&range.file_id)?;
                let (line, col) = positions.get(file)?.position(range.start);
                Some((Some(file.to_owned()), line, col))
            })
            .unwrap_or((None, 0, 0));
        let child = ModuleGraphInstance {
            name: node.name.clone(),
            module_type: clean_name(&node.definition_name).to_owned(),
            file,
            line,
            col,
        };
        for definition in definitions
            .iter_mut()
            .filter(|definition| &definition.id == owner_key)
        {
            if let Some(existing) = definition.children.iter_mut().find(|existing| {
                existing.name == child.name && existing.module_type == child.module_type
            }) {
                if node.is_uninstantiated {
                    *existing = child.clone();
                }
            } else {
                definition.children.push(child.clone());
            }
        }
    }
    for definition in &mut definitions {
        definition.children.sort_by(|left, right| {
            (
                &left.file,
                left.line,
                left.col,
                &left.name,
                &left.module_type,
            )
                .cmp(&(
                    &right.file,
                    right.line,
                    right.col,
                    &right.name,
                    &right.module_type,
                ))
        });
    }

    for node in &snapshot.semantic_nodes {
        if !matches!(
            node.kind,
            SemanticKind::Port
                | SemanticKind::Parameter
                | SemanticKind::Net
                | SemanticKind::Variable
                | SemanticKind::Array
        ) || node.name.is_empty()
        {
            continue;
        }
        let mut ancestor = node
            .parent_id
            .and_then(|id| semantic_by_id.get(&id).copied());
        let owner_id = loop {
            let Some(parent) = ancestor else { break None };
            match parent.kind {
                SemanticKind::Definition => break Some(parent.id),
                SemanticKind::Instance if !parent.definition_name.is_empty() => {
                    break parent.target_id;
                }
                SemanticKind::Subroutine => break None,
                _ => {
                    ancestor = parent
                        .parent_id
                        .and_then(|id| semantic_by_id.get(&id).copied());
                }
            }
        };
        let Some(owner_key) = owner_id.and_then(|id| definition_keys.get(&id)) else {
            continue;
        };
        let Some(location) = node.range.and_then(|range| {
            let file = *files.get(&range.file_id)?;
            let positions = positions.get(file)?;
            let (line, col) = positions.position(range.start);
            let (end_line, end_col) = positions.position(range.end);
            Some(ModuleGraphLocation {
                file: file.to_owned(),
                line,
                col,
                end_line,
                end_col,
            })
        }) else {
            continue;
        };
        let (display_type, display_shape, detail) = node
            .range
            .and_then(|range| {
                let file = *files.get(&range.file_id)?;
                let text = *texts.get(file)?;
                source_decl_type(text, range, &node.name)
            })
            .unwrap_or((None, ModuleGraphTypeShape::default(), None));
        let source_type = declaration_types
            .get(&(
                location.file.clone(),
                location.line,
                location.col,
                node.name.clone(),
            ))
            .cloned()
            .unwrap_or_default();
        for definition in definitions
            .iter_mut()
            .filter(|definition| &definition.id == owner_key)
        {
            match node.kind {
                SemanticKind::Port => {
                    if definition.ports.iter().any(|item| {
                        item.name == node.name && item.location.as_ref() == Some(&location)
                    }) {
                        continue;
                    }
                    let direction = if node.is_input {
                        Direction::Input
                    } else if node.is_output {
                        Direction::Output
                    } else if node.is_inout {
                        Direction::Inout
                    } else {
                        Direction::None
                    };
                    definition.ports.push(ModuleGraphPort {
                        name: node.name.clone(),
                        direction,
                        ty: source_type.clone(),
                        detail: detail.clone(),
                        location: Some(location.clone()),
                        display_type: display_type.clone(),
                        display_shape,
                    });
                }
                SemanticKind::Parameter => {
                    if definition.params.iter().any(|item| {
                        item.name == node.name && item.location.as_ref() == Some(&location)
                    }) {
                        continue;
                    }
                    definition.params.push(ModuleGraphParameter {
                        name: node.name.clone(),
                        ty: source_type.clone(),
                        local: node.is_local,
                        detail: detail.clone(),
                        location: Some(location.clone()),
                        display_type: display_type.clone(),
                        display_shape,
                    });
                }
                SemanticKind::Net | SemanticKind::Variable | SemanticKind::Array => {
                    if definition.signals.iter().any(|item| {
                        item.name == node.name && item.location.as_ref() == Some(&location)
                    }) {
                        continue;
                    }
                    definition.signals.push(ModuleGraphSignal {
                        name: node.name.clone(),
                        kind: if node.kind == SemanticKind::Net {
                            source_net_kind(node.subkind).to_owned()
                        } else if node.kind == SemanticKind::Array {
                            "array".to_owned()
                        } else {
                            "var".to_owned()
                        },
                        ty: source_type.clone(),
                        detail: detail.clone(),
                        location: Some(location.clone()),
                        display_type: display_type.clone(),
                        display_shape,
                    });
                }
                _ => {}
            }
        }
    }
    let types_by_id: HashMap<_, _> = snapshot.types.iter().map(|ty| (ty.id, ty)).collect();
    let mut elaborated_types = Vec::new();
    for node in &snapshot.semantic_nodes {
        if !matches!(
            node.kind,
            SemanticKind::Port
                | SemanticKind::Parameter
                | SemanticKind::Net
                | SemanticKind::Variable
                | SemanticKind::Array
        ) || node.name.is_empty()
        {
            continue;
        }
        let mut ancestor = node
            .parent_id
            .and_then(|id| semantic_by_id.get(&id).copied());
        let mut instance_names = Vec::new();
        while let Some(parent) = ancestor {
            if parent.kind == SemanticKind::Instance && !parent.name.is_empty() {
                instance_names.push(parent.name.as_str());
            }
            ancestor = parent
                .parent_id
                .and_then(|id| semantic_by_id.get(&id).copied());
        }
        if instance_names.is_empty() {
            continue;
        }
        instance_names.reverse();
        let packed_ranges = node
            .type_id
            .map(|id| slang_packed_ranges(id, &types_by_id, &snapshot.type_ranges))
            .unwrap_or_default();
        if packed_ranges.is_empty() {
            continue;
        }
        let item = ModuleGraphElaboratedType {
            instance: instance_names.join("."),
            name: node.name.clone(),
            packed_ranges,
        };
        if !elaborated_types.contains(&item) {
            elaborated_types.push(item);
        }
    }
    ModuleGraph {
        definitions,
        elaborated_types,
    }
}

fn source_net_kind(subkind: u32) -> &'static str {
    match subkind {
        128 => "wire",
        129 => "wand",
        130 => "wor",
        131 => "tri",
        132 => "triand",
        133 => "trior",
        134 => "tri0",
        135 => "tri1",
        136 => "trireg",
        137 => "supply0",
        138 => "supply1",
        139 => "uwire",
        _ => "net",
    }
}

fn slang_packed_ranges(
    mut type_id: u64,
    types: &HashMap<u64, &llg::ffi::slang::Type>,
    ranges: &[llg::ffi::slang::TypeRange],
) -> Vec<Option<ModuleGraphPackedRange>> {
    let mut result = Vec::new();
    for _ in 0..64 {
        let Some(ty) = types.get(&type_id).copied() else {
            break;
        };
        let start = usize::try_from(ty.range_start).unwrap_or(usize::MAX);
        let count = usize::try_from(ty.range_count).unwrap_or(usize::MAX);
        if let Some(end) = start.checked_add(count) {
            if let Some(type_ranges) = ranges.get(start..end) {
                result.extend(type_ranges.iter().filter_map(|range| {
                    (range.kind == llg::ffi::slang::TypeRangeKind::Packed).then_some(Some(
                        ModuleGraphPackedRange {
                            left: range.left.into(),
                            right: range.right.into(),
                        },
                    ))
                }));
            }
        }
        let Some(element) = ty.element_type_id else {
            break;
        };
        type_id = element;
    }
    result
}

fn source_decl_type(
    text: &str,
    range: llg::ffi::slang::SourceRange,
    name: &str,
) -> Option<(Option<String>, ModuleGraphTypeShape, Option<String>)> {
    let start = usize::try_from(range.start).ok()?.min(text.len());
    let end = usize::try_from(range.end).ok()?.min(text.len());
    if start > end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |at| end + at);
    let mut prefix_text = text[line_start..start]
        .rsplit(['(', ',', ';'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    // Slang locates a declarator at its name. For a declaration whose shared
    // data type ends on preceding lines, the name's line contains only
    // indentation. Walk through bracket-only continuation lines until the
    // base type is owned as well.
    let has_base_type = |value: &str| {
        let mut bracket_depth = 0_u32;
        value.chars().any(|character| match character {
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                false
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                false
            }
            _ => bracket_depth == 0 && (character.is_alphanumeric() || character == '_'),
        })
    };
    let mut previous_end = line_start.saturating_sub(1);
    while !has_base_type(&prefix_text) && previous_end > 0 {
        let previous_start = text[..previous_end].rfind('\n').map_or(0, |at| at + 1);
        let previous = text[previous_start..previous_end]
            .rsplit(['(', ',', ';'])
            .next()
            .unwrap_or_default()
            .trim_end_matches('\r')
            .trim();
        if !previous.is_empty() {
            prefix_text = if prefix_text.is_empty() {
                previous.to_owned()
            } else {
                format!("{previous} {prefix_text}")
            };
        }
        previous_end = previous_start.saturating_sub(1);
    }
    let mut prefix = prefix_text.to_owned();
    for keyword in ["input", "output", "inout", "parameter", "localparam"] {
        if prefix
            .strip_prefix(keyword)
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        {
            prefix = prefix[keyword.len()..].trim_start().to_owned();
            break;
        }
    }
    let suffix = text[end..line_end]
        .split(['=', ',', ';', ')'])
        .next()
        .unwrap_or_default()
        .trim();
    let display = [prefix.as_str(), suffix]
        .into_iter()
        .filter(|part| !part.is_empty() && *part != name)
        .collect::<Vec<_>>()
        .join(" ");
    if display.is_empty() {
        return None;
    }
    let brackets = |value: &str| value.bytes().filter(|byte| *byte == b'[').count();
    let unpacked_dimensions = brackets(suffix);
    let packed_dimensions = brackets(&prefix);
    let detail = [prefix_text.as_str(), name, suffix]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Some((
        Some(display),
        ModuleGraphTypeShape {
            packed_dimensions,
            unpacked_dimensions,
        },
        (!detail.is_empty()).then_some(detail),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_decl_type_keeps_a_shared_type_from_the_previous_line() {
        let text = "output logic\n    [WIDTH-1:0]\n    payload [0:3],\n";
        let start = text.find("payload").expect("payload offset");
        let end = start + "payload".len();
        let (display, shape, detail) = source_decl_type(
            text,
            llg::ffi::slang::SourceRange {
                file_id: 1,
                start: start as u64,
                end: end as u64,
            },
            "payload",
        )
        .expect("declaration type");

        assert_eq!(display.as_deref(), Some("logic [WIDTH-1:0] [0:3]"));
        assert_eq!(shape.packed_dimensions, 1);
        assert_eq!(shape.unpacked_dimensions, 1);
        assert_eq!(
            detail.as_deref(),
            Some("output logic [WIDTH-1:0] payload [0:3]")
        );
    }
}
