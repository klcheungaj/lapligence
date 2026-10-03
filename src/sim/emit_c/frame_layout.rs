//! Explicit storage layouts for generated coroutine functions.

use std::collections::{BTreeMap, BTreeSet};

use crate::sim::execution::CallMechanism;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FrameStorage {
    CStack,
    CoFrame,
}

pub(super) fn requires_arena(frame_upper_bound: usize, embed_limit: usize) -> bool {
    frame_upper_bound > embed_limit
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Field {
    ty: String,
    name: String,
    size: usize,
    align: usize,
    hot: bool,
    always_frame: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CallSlot {
    pub(super) resume: u32,
    pub(super) member: String,
    /// Member path relative to the frame object, suitable for `offsetof`.
    pub(super) path: String,
    pub(super) callee_type: String,
    pub(super) mechanism: CallMechanism,
    pub(super) callee_upper_bound: usize,
}

#[derive(Clone, Debug, Default)]
struct Block {
    parent: Option<usize>,
    fields: Vec<Field>,
    calls: Vec<usize>,
    children: Vec<usize>,
    contains_resume: bool,
}

/// Builder for one generated function's stack or explicit coroutine storage.
///
/// Coroutine storage follows the emitted C block tree. Parent storage remains
/// live while a child executes, while sibling child structs are members of a
/// union and therefore share storage. All declarations and call slots bind to
/// the current block through this builder.
#[derive(Clone, Debug)]
pub(super) struct FrameLayout {
    storage: FrameStorage,
    blocks: Vec<Block>,
    current: usize,
    names: BTreeSet<String>,
    accesses: BTreeMap<String, String>,
    calls: Vec<CallSlot>,
    paths_finalized: bool,
}

impl FrameLayout {
    pub(super) fn new(storage: FrameStorage) -> Self {
        Self {
            storage,
            blocks: vec![Block::default()],
            current: 0,
            names: BTreeSet::new(),
            accesses: BTreeMap::new(),
            calls: Vec::new(),
            paths_finalized: false,
        }
    }

    pub(super) fn storage(&self) -> FrameStorage {
        self.storage
    }

    pub(super) fn begin_block(&mut self) {
        if self.storage == FrameStorage::CStack {
            return;
        }
        let child = self.blocks.len();
        self.blocks.push(Block {
            parent: Some(self.current),
            ..Block::default()
        });
        self.blocks[self.current].children.push(child);
        self.current = child;
    }

    pub(super) fn current_block(&self) -> usize {
        self.current
    }

    pub(super) fn exited_blocks(&self, target: usize) -> Vec<usize> {
        let mut blocks = Vec::new();
        let mut cursor = self.current;
        while cursor != target {
            blocks.push(cursor);
            let Some(parent) = self.blocks[cursor].parent else {
                break;
            };
            cursor = parent;
        }
        blocks
    }

    /// Only a sibling union's struct is independently poisonable. A flattened
    /// child shares its struct with still-live enclosing fields.
    pub(super) fn overlay_paths(&self) -> BTreeMap<usize, String> {
        fn collect(
            layout: &FrameLayout,
            block: usize,
            prefix: &str,
            storage: &[bool],
            paths: &mut BTreeMap<usize, String>,
        ) {
            let chain = layout.flat_chain(block, storage);
            let terminal = *chain.last().expect("flat block chain is nonempty");
            for child in layout.storage_children(terminal, storage) {
                let path = format!("{prefix}u{terminal}.b{child}");
                paths.insert(child, path.clone());
                collect(layout, child, &format!("{path}."), storage, paths);
            }
        }
        let mut paths = BTreeMap::new();
        if self.storage == FrameStorage::CoFrame {
            collect(self, 0, "", &self.storage_map(), &mut paths);
        }
        paths
    }

    pub(super) fn end_block(&mut self) -> Result<(), String> {
        if self.storage == FrameStorage::CStack {
            return Ok(());
        }
        self.current = self.blocks[self.current]
            .parent
            .ok_or_else(|| "coroutine frame block tree closed past its root".to_owned())?;
        Ok(())
    }

    pub(super) fn finish_blocks(&self) -> Result<(), String> {
        if self.storage == FrameStorage::CoFrame && self.current != 0 {
            return Err(format!(
                "coroutine frame block tree has {} unclosed block(s)",
                self.block_depth(self.current)
            ));
        }
        Ok(())
    }

    /// Record a resume point in the active C block and every enclosing scope.
    /// A declaration is frame-resident exactly when its declaring scope is
    /// marked by this propagation.
    pub(super) fn mark_resume(&mut self) {
        if self.storage == FrameStorage::CStack {
            return;
        }
        let mut block = Some(self.current);
        while let Some(index) = block {
            if self.blocks[index].contains_resume {
                break;
            }
            self.blocks[index].contains_resume = true;
            block = self.blocks[index].parent;
        }
    }

    /// Register one typed declaration and return the expression used to access it.
    pub(super) fn declare(&mut self, ty: &str, name: &str) -> Result<String, String> {
        self.register_field(ty, name, false)
    }

    /// Register entry storage supplied by the caller. Unlike a declaration,
    /// arguments must remain addressable through the ABI frame even when the
    /// selected coroutine body itself has no resume point.
    pub(super) fn declare_required(&mut self, ty: &str, name: &str) -> Result<String, String> {
        self.register_field(ty, name, true)
    }

    fn register_field(
        &mut self,
        ty: &str,
        name: &str,
        always_frame: bool,
    ) -> Result<String, String> {
        if self.storage == FrameStorage::CStack {
            return Ok(name.to_owned());
        }
        if !self.names.insert(name.to_owned()) {
            return Err(format!("duplicate coroutine frame field `{name}`"));
        }
        let (size, align) = lp64_layout(ty)?;
        // The final path depends on whether sibling blocks retain any frame
        // storage. Defer constructing it until the whole block tree is known.
        self.accesses.insert(name.to_owned(), name.to_owned());
        self.blocks[self.current].fields.push(Field {
            ty: ty.to_owned(),
            name: name.to_owned(),
            size,
            align,
            hot: false,
            always_frame,
        });
        Ok(format!("F->{name}"))
    }

    pub(super) fn field_access(&self, name: &str) -> Option<&str> {
        self.accesses.get(name).map(String::as_str)
    }

    pub(super) fn mark_hot(&mut self, name: &str) {
        for block in &mut self.blocks {
            if let Some(field) = block.fields.iter_mut().find(|field| field.name == name) {
                field.hot = true;
                return;
            }
        }
    }

    pub(super) fn add_call(
        &mut self,
        resume: u32,
        callee_type: &str,
        mechanism: CallMechanism,
        callee_upper_bound: usize,
    ) -> Result<CallSlot, String> {
        let prefix = match mechanism {
            CallMechanism::Polled { .. } => "c",
            CallMechanism::Anchored => "a",
            CallMechanism::Arena => "arena",
        };
        let member = format!("{prefix}{}", self.calls.len());
        let path = self.path_to(self.current, &format!("calls.{member}"));
        if mechanism != CallMechanism::Arena && !self.names.insert(path.clone()) {
            return Err(format!("duplicate coroutine call frame member `{path}`"));
        }
        let slot = CallSlot {
            resume,
            member,
            path,
            callee_type: callee_type.to_owned(),
            mechanism,
            callee_upper_bound,
        };
        let index = self.calls.len();
        self.calls.push(slot.clone());
        self.blocks[self.current].calls.push(index);
        self.mark_resume();
        Ok(slot)
    }

    pub(super) fn calls(&self) -> &[CallSlot] {
        &self.calls
    }

    /// Finalize member paths after the complete sibling set is known.
    ///
    /// Emission initially uses the uncompressed structural path so declarations
    /// can be referenced immediately. This returns the changed paths for the
    /// function body rewrite and publishes the same flattened paths to metadata.
    pub(super) fn finalize_paths(&mut self) -> Result<Vec<(String, String)>, String> {
        self.finish_blocks()?;
        if self.paths_finalized {
            return Ok(Vec::new());
        }
        let storage = self.storage_map();
        let mut field_paths = BTreeMap::new();
        let mut call_paths = BTreeMap::new();
        self.assign_flat_paths(0, "", &storage, &mut field_paths, &mut call_paths);

        let mut changed = Vec::new();
        let names = self.accesses.keys().cloned().collect::<Vec<_>>();
        for name in names {
            if let Some(flattened) = field_paths.remove(&name) {
                self.accesses.insert(name, flattened);
            } else {
                self.accesses.remove(&name);
            }
        }
        for (index, call) in self.calls.iter_mut().enumerate() {
            let flattened = call_paths
                .remove(&index)
                .ok_or_else(|| format!("missing flattened path for call slot {index}"))?;
            if call.path != flattened {
                changed.push((call.path.clone(), flattened.clone()));
                call.path = flattened;
            }
        }
        changed.sort_by_key(|(original, _)| std::cmp::Reverse(original.len()));
        self.paths_finalized = true;
        Ok(changed)
    }

    /// Conservative LP64 size used for D19. Every embedded call is counted as
    /// anchored, independently of the mechanism selected by the first pass.
    /// Sibling C blocks contribute their maximum rather than their sum.
    pub(super) fn upper_bound(&self) -> Result<usize, String> {
        if self.storage == FrameStorage::CStack {
            return Ok(0);
        }
        let storage = self.storage_map();
        let (size, align) = self.flat_block_layout(0, 8, 4, &storage)?;
        align_up(size, align)
    }

    pub(super) fn render_typedef(&self, frame_type: &str) -> Result<String, String> {
        if self.storage != FrameStorage::CoFrame {
            return Ok(String::new());
        }
        self.finish_blocks()?;
        let storage = self.storage_map();
        let mut out = String::from("typedef struct {\n    llg_co_frame_t co;\n");
        self.render_flat_contents(0, 1, &storage, &mut out);
        out.push_str(&format!("}} {frame_type};\n"));
        Ok(out)
    }

    fn path_to(&self, block: usize, suffix: &str) -> String {
        let mut ancestors = Vec::new();
        let mut cursor = block;
        while let Some(parent) = self.blocks[cursor].parent {
            ancestors.push((parent, cursor));
            cursor = parent;
        }
        ancestors.reverse();
        let mut path = String::new();
        for (parent, child) in ancestors {
            path.push_str(&format!("u{parent}.b{child}."));
        }
        path.push_str(suffix);
        path
    }

    fn block_depth(&self, mut block: usize) -> usize {
        let mut depth = 0;
        while let Some(parent) = self.blocks[block].parent {
            depth += 1;
            block = parent;
        }
        depth
    }

    fn embedded_calls(&self, block: usize) -> impl Iterator<Item = &CallSlot> {
        self.blocks[block]
            .calls
            .iter()
            .map(|index| &self.calls[*index])
            .filter(|call| call.mechanism != CallMechanism::Arena)
    }

    fn storage_map(&self) -> Vec<bool> {
        let mut storage = self
            .blocks
            .iter()
            .map(|block| {
                block
                    .fields
                    .iter()
                    .any(|field| block.contains_resume || field.always_frame)
                    || block
                        .calls
                        .iter()
                        .any(|call| self.calls[*call].mechanism != CallMechanism::Arena)
            })
            .collect::<Vec<_>>();
        for child in (1..self.blocks.len()).rev() {
            if storage[child] {
                if let Some(parent) = self.blocks[child].parent {
                    storage[parent] = true;
                }
            }
        }
        storage
    }

    fn storage_children(&self, block: usize, storage: &[bool]) -> Vec<usize> {
        self.blocks[block]
            .children
            .iter()
            .copied()
            .filter(|child| storage[*child])
            .collect()
    }

    /// Return blocks merged into one struct level and the final block whose
    /// children either end the level or require a sibling overlay.
    fn flat_chain(&self, block: usize, storage: &[bool]) -> Vec<usize> {
        let mut chain = vec![block];
        loop {
            let current = *chain.last().expect("flat block chain is nonempty");
            let children = self.storage_children(current, storage);
            if children.len() != 1 {
                return chain;
            }
            chain.push(children[0]);
        }
    }

    fn assign_flat_paths(
        &self,
        block: usize,
        prefix: &str,
        storage: &[bool],
        fields: &mut BTreeMap<String, String>,
        calls: &mut BTreeMap<usize, String>,
    ) {
        let chain = self.flat_chain(block, storage);
        for &item in &chain {
            for field in &self.blocks[item].fields {
                if self.blocks[item].contains_resume || field.always_frame {
                    fields.insert(field.name.clone(), format!("{prefix}{}", field.name));
                }
            }
            for &call in &self.blocks[item].calls {
                calls.insert(
                    call,
                    format!("{prefix}calls{item}.{}", self.calls[call].member),
                );
            }
        }
        let terminal = *chain.last().expect("flat block chain is nonempty");
        let children = self.storage_children(terminal, storage);
        debug_assert!(children.len() != 1);
        for child in children {
            self.assign_flat_paths(
                child,
                &format!("{prefix}u{terminal}.b{child}."),
                storage,
                fields,
                calls,
            );
        }
    }

    fn render_flat_contents(
        &self,
        block: usize,
        indent: usize,
        storage: &[bool],
        out: &mut String,
    ) {
        let pad = "    ".repeat(indent);
        let chain = self.flat_chain(block, storage);
        for hot in [true, false] {
            for field in chain
                .iter()
                .flat_map(|item| {
                    self.blocks[*item]
                        .fields
                        .iter()
                        .filter(|field| self.blocks[*item].contains_resume || field.always_frame)
                })
                .filter(|field| field.hot == hot)
            {
                out.push_str(&pad);
                out.push_str(&declaration(&field.ty, &field.name));
                out.push_str(";\n");
            }
        }
        for &item in &chain {
            let embedded = self.embedded_calls(item).collect::<Vec<_>>();
            if !embedded.is_empty() {
                out.push_str(&format!("{pad}union {{\n"));
                for call in embedded {
                    match call.mechanism {
                        CallMechanism::Polled { .. } => out
                            .push_str(&format!("{pad}    {} {};\n", call.callee_type, call.member)),
                        CallMechanism::Anchored => out.push_str(&format!(
                            "{pad}    LLG_CO_ANCHORED({}) {};\n",
                            call.callee_type, call.member
                        )),
                        CallMechanism::Arena => unreachable!(),
                    }
                }
                out.push_str(&format!("{pad}}} calls{item};\n"));
            }
        }
        let terminal = *chain.last().expect("flat block chain is nonempty");
        let children = self.storage_children(terminal, storage);
        debug_assert!(children.len() != 1);
        if !children.is_empty() {
            out.push_str(&format!("{pad}union {{\n"));
            for child in children {
                out.push_str(&format!("{pad}    struct {{\n"));
                self.render_flat_contents(child, indent + 2, storage, out);
                out.push_str(&format!("{pad}    }} b{child};\n"));
            }
            out.push_str(&format!("{pad}}} u{terminal};\n"));
        }
    }

    fn flat_block_layout(
        &self,
        block: usize,
        mut size: usize,
        mut max_align: usize,
        storage: &[bool],
    ) -> Result<(usize, usize), String> {
        let chain = self.flat_chain(block, storage);
        for hot in [true, false] {
            for field in chain
                .iter()
                .flat_map(|item| {
                    self.blocks[*item]
                        .fields
                        .iter()
                        .filter(|field| self.blocks[*item].contains_resume || field.always_frame)
                })
                .filter(|field| field.hot == hot)
            {
                size = align_up(size, field.align)?;
                size = size
                    .checked_add(field.size)
                    .ok_or_else(|| "coroutine frame size overflowed".to_owned())?;
                max_align = max_align.max(field.align);
            }
        }
        for &item in &chain {
            if let Some(call_size) = self
                .embedded_calls(item)
                .map(|call| call.callee_upper_bound.saturating_add(16))
                .max()
            {
                size = align_up(size, 16)?;
                size = size
                    .checked_add(call_size)
                    .ok_or_else(|| "coroutine frame size overflowed".to_owned())?;
                max_align = max_align.max(16);
            }
        }

        let terminal = *chain.last().expect("flat block chain is nonempty");
        let children = self.storage_children(terminal, storage);
        debug_assert!(children.len() != 1);
        if !children.is_empty() {
            let mut union_size = 0;
            let mut union_align = 1;
            for child in children {
                let (candidate_size, candidate_align) =
                    self.flat_block_layout(child, 0, 1, storage)?;
                union_size = union_size.max(align_up(candidate_size, candidate_align)?);
                union_align = union_align.max(candidate_align);
            }
            size = align_up(size, union_align)?;
            size = size
                .checked_add(union_size)
                .ok_or_else(|| "coroutine frame size overflowed".to_owned())?;
            max_align = max_align.max(union_align);
        }
        Ok((align_up(size, max_align)?, max_align))
    }
}

pub(super) fn declaration(ty: &str, name: &str) -> String {
    if let Some(array) = ty.find('[') {
        format!("{} {name}{}", ty[..array].trim_end(), &ty[array..])
    } else {
        format!("{ty} {name}")
    }
}

fn align_up(value: usize, align: usize) -> Result<usize, String> {
    let mask = align
        .checked_sub(1)
        .ok_or_else(|| "zero frame-field alignment".to_owned())?;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or_else(|| "coroutine frame alignment overflowed".to_owned())
}

/// LP64 layouts of types that can be emitted as procedure-local storage.
/// Aggregate runtime types are deliberately conservative upper bounds.
fn lp64_layout(ty: &str) -> Result<(usize, usize), String> {
    let ty = ty.trim();
    if let Some(array) = ty.find('[') {
        let (element_size, element_align) = lp64_layout(ty[..array].trim())?;
        let count = ty[array + 1..]
            .strip_suffix(']')
            .ok_or_else(|| format!("unsupported frame array type `{ty}`"))?
            .parse::<usize>()
            .map_err(|_| format!("unsupported frame array bound in `{ty}`"))?;
        return Ok((
            element_size
                .checked_mul(count)
                .ok_or_else(|| "coroutine frame array size overflowed".to_owned())?,
            element_align,
        ));
    }
    let plain = ty.strip_prefix("const ").unwrap_or(ty);
    let plain = plain.strip_suffix(" const").unwrap_or(plain).trim();
    if plain.contains('*') || plain == "size_t" || plain == "uint64_t" || plain == "int64_t" {
        return Ok((8, 8));
    }
    let layout = match plain {
        "int" | "unsigned" | "uint32_t" | "int32_t" => (4, 4),
        "uint16_t" | "int16_t" => (2, 2),
        "uint8_t" | "int8_t" | "char" => (1, 1),
        "double" => (8, 8),
        "sv4_t" => (32, 8),
        "llg_value_scope_t" => (80, 8),
        "llg_string_t" => (32, 8),
        "sv4_select_plan_t" => (20, 4),
        "llg_event_t" => (8, 8),
        "llg_event_spec_t" => (16, 8),
        "llg_fmt_arg_t" => (48, 8),
        "llg_wait_dependency_t" => (32, 8),
        "llg_expr_event_spec_t" => (104, 8),
        "llg_ref_t" => (112, 8),
        "llg_vpi_arg_t" => (64, 8),
        "llg_force_read_t"
        | "llg_force_part_t"
        | "llg_file_input_target_t"
        | "llg_display_read_t"
        | "llg_wait_src_t" => (64, 8),
        other => return Err(format!("unknown LP64 coroutine frame field type `{other}`")),
    };
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_spec_layout_matches_the_lp64_runtime_contract() {
        assert_eq!(lp64_layout("llg_event_spec_t").unwrap(), (16, 8));
    }

    #[test]
    fn coframe_starts_with_header_and_rejects_duplicate_fields() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        assert_eq!(layout.declare("uint64_t", "ticks").unwrap(), "F->ticks");
        assert!(layout.declare("int", "ticks").is_err());
        layout.mark_resume();
        let rendered = layout.render_typedef("p_frame_t").unwrap();
        assert!(rendered.starts_with("typedef struct {\n    llg_co_frame_t co;\n"));
    }

    #[test]
    fn resume_free_scope_keeps_declarations_off_the_frame() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.declare("uint64_t", "root_local").unwrap();
        layout.begin_block();
        layout.declare("sv4_t", "child_local").unwrap();
        layout.end_block().unwrap();
        layout.finalize_paths().unwrap();

        assert_eq!(
            layout.render_typedef("leaf_frame_t").unwrap(),
            "typedef struct {\n    llg_co_frame_t co;\n} leaf_frame_t;\n"
        );
        assert_eq!(layout.field_access("root_local"), None);
        assert_eq!(layout.field_access("child_local"), None);
        assert_eq!(layout.upper_bound().unwrap(), 8);
    }

    #[test]
    fn only_the_sibling_scope_with_a_resume_is_frame_resident() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.declare("uint64_t", "root").unwrap();
        layout.begin_block();
        layout.declare("sv4_t", "leaf_local").unwrap();
        layout.end_block().unwrap();
        layout.begin_block();
        layout.declare("sv4_t", "resumed").unwrap();
        layout.mark_resume();
        layout.end_block().unwrap();
        layout.finalize_paths().unwrap();

        assert_eq!(layout.field_access("root"), Some("root"));
        assert_eq!(layout.field_access("leaf_local"), None);
        assert_eq!(layout.field_access("resumed"), Some("resumed"));
        assert_eq!(layout.upper_bound().unwrap(), 48);
    }

    #[test]
    fn caller_supplied_arguments_remain_in_a_resume_free_callee_frame() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.declare_required("sv4_t", "a0").unwrap();
        layout.declare_required("int", "depth").unwrap();
        layout.declare("uint64_t", "local").unwrap();
        layout.finalize_paths().unwrap();

        assert_eq!(layout.field_access("a0"), Some("a0"));
        assert_eq!(layout.field_access("depth"), Some("depth"));
        assert_eq!(layout.field_access("local"), None);
        assert_eq!(layout.upper_bound().unwrap(), 48);
    }

    #[test]
    fn sibling_blocks_overlay_fields_and_call_slots() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.begin_block();
        assert_eq!(layout.declare("uint64_t", "left").unwrap(), "F->left");
        layout
            .add_call(1, "small_frame_t", CallMechanism::Polled { depth: 1 }, 64)
            .unwrap();
        layout.end_block().unwrap();
        layout.begin_block();
        assert_eq!(layout.declare("sv4_t", "right").unwrap(), "F->right");
        layout
            .add_call(2, "deep_frame_t", CallMechanism::Anchored, 80)
            .unwrap();
        layout.end_block().unwrap();
        layout.finalize_paths().unwrap();
        let rendered = layout.render_typedef("caller_frame_t").unwrap();
        assert!(rendered.contains("union {\n        struct {\n            uint64_t left;"));
        assert!(rendered.contains("small_frame_t c0;"));
        assert!(rendered.contains("LLG_CO_ANCHORED(deep_frame_t) a1;"));
        assert!(rendered.contains("} calls1;"));
        assert!(rendered.contains("} calls2;"));
        assert!(rendered.contains("} b1;"));
        assert!(rendered.contains("} b2;"));
        assert_eq!(layout.field_access("left"), Some("u0.b1.left"));
        assert_eq!(layout.field_access("right"), Some("u0.b2.right"));
        assert_eq!(layout.upper_bound().unwrap(), 144);
    }

    #[test]
    fn nested_blocks_add_to_live_parent_storage() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.declare("uint64_t", "root").unwrap();
        layout.begin_block();
        layout.declare("sv4_t", "child").unwrap();
        layout.mark_resume();
        layout.end_block().unwrap();
        layout.finalize_paths().unwrap();
        let rendered = layout.render_typedef("f_t").unwrap();
        assert!(!rendered.contains("union {"), "{rendered}");
        assert_eq!(layout.field_access("child"), Some("child"));
        assert_eq!(layout.upper_bound().unwrap(), 48);
    }

    #[test]
    fn single_child_block_chain_has_constant_definition_depth() {
        const DEPTH: usize = 128;
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        for _ in 0..DEPTH {
            layout.begin_block();
        }
        layout.declare("uint64_t", "leaf").unwrap();
        layout.mark_resume();
        for _ in 0..DEPTH {
            layout.end_block().unwrap();
        }
        layout.finalize_paths().unwrap();
        let rendered = layout.render_typedef("deep_frame_t").unwrap();
        let definition_depth =
            rendered.matches("struct {").count() + rendered.matches("union {").count();
        assert_eq!(definition_depth, 1, "{rendered}");
        assert_eq!(layout.field_access("leaf"), Some("leaf"));
    }

    #[test]
    fn hot_fields_are_first_and_stable() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.declare("int", "cold0").unwrap();
        layout.declare("int", "hot0").unwrap();
        layout.begin_block();
        layout.declare("int", "cold1").unwrap();
        layout.declare("int", "hot1").unwrap();
        layout.mark_resume();
        layout.end_block().unwrap();
        layout.mark_hot("hot0");
        layout.mark_hot("hot1");
        layout.finalize_paths().unwrap();
        let first = layout.render_typedef("f_t").unwrap();
        let second = layout.render_typedef("f_t").unwrap();
        assert_eq!(first, second);
        assert!(first.find("int hot0;").unwrap() < first.find("int hot1;").unwrap());
        assert!(first.find("int hot1;").unwrap() < first.find("int cold0;").unwrap());
        assert!(first.find("int cold0;").unwrap() < first.find("int cold1;").unwrap());
    }

    #[test]
    fn unbalanced_block_tree_is_an_error() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.begin_block();
        assert!(layout.render_typedef("f_t").is_err());
        layout.end_block().unwrap();
        assert!(layout.end_block().is_err());
    }

    #[test]
    fn upper_bound_counts_every_site_as_anchored() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout.declare("uint64_t", "ticks").unwrap();
        layout
            .add_call(1, "callee_frame_t", CallMechanism::Polled { depth: 1 }, 64)
            .unwrap();
        assert_eq!(layout.upper_bound().unwrap(), 96);
    }

    #[test]
    fn embed_limit_is_inclusive() {
        let limit = crate::sim::execution::DEFAULT_EMBED_LIMIT;
        assert!(!requires_arena(limit - 1, limit));
        assert!(!requires_arena(limit, limit));
        assert!(requires_arena(limit + 1, limit));
    }
}
