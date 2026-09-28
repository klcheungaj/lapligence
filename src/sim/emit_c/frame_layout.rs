//! Explicit storage layouts for generated coroutine functions.

use std::collections::BTreeSet;

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CallSlot {
    pub(super) resume: u32,
    pub(super) member: String,
    pub(super) callee_type: String,
    pub(super) mechanism: CallMechanism,
    pub(super) callee_upper_bound: usize,
}

/// Builder for one generated function's stack or explicit coroutine storage.
///
/// Part B can replace the flat `fields` list with a lexical block tree without
/// changing declaration call sites: all names and accesses already flow through
/// [`FrameLayout::declare`].
#[derive(Clone, Debug)]
pub(super) struct FrameLayout {
    storage: FrameStorage,
    fields: Vec<Field>,
    names: BTreeSet<String>,
    calls: Vec<CallSlot>,
}

impl FrameLayout {
    pub(super) fn new(storage: FrameStorage) -> Self {
        Self {
            storage,
            fields: Vec::new(),
            names: BTreeSet::new(),
            calls: Vec::new(),
        }
    }

    pub(super) fn storage(&self) -> FrameStorage {
        self.storage
    }

    /// Register one typed declaration and return the expression used to access it.
    pub(super) fn declare(&mut self, ty: &str, name: &str) -> Result<String, String> {
        if self.storage == FrameStorage::CStack {
            return Ok(name.to_owned());
        }
        if !self.names.insert(name.to_owned()) {
            return Err(format!("duplicate coroutine frame field `{name}`"));
        }
        let (size, align) = lp64_layout(ty)?;
        self.fields.push(Field {
            ty: ty.to_owned(),
            name: name.to_owned(),
            size,
            align,
        });
        Ok(format!("F->{name}"))
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
        if mechanism != CallMechanism::Arena && !self.names.insert(format!("calls.{member}")) {
            return Err(format!("duplicate coroutine call frame member `{member}`"));
        }
        let slot = CallSlot {
            resume,
            member,
            callee_type: callee_type.to_owned(),
            mechanism,
            callee_upper_bound,
        };
        self.calls.push(slot.clone());
        Ok(slot)
    }

    pub(super) fn calls(&self) -> &[CallSlot] {
        &self.calls
    }

    pub(super) fn field_names(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|field| field.name.as_str())
    }

    /// Conservative LP64 size used for D19. Every call is counted as anchored,
    /// independently of the mechanism selected by the first analysis pass.
    pub(super) fn upper_bound(&self) -> Result<usize, String> {
        if self.storage == FrameStorage::CStack {
            return Ok(0);
        }
        let mut size = 8usize;
        let mut max_align = 4usize;
        for field in &self.fields {
            size = align_up(size, field.align)?;
            size = size
                .checked_add(field.size)
                .ok_or_else(|| "coroutine frame size overflowed".to_owned())?;
            max_align = max_align.max(field.align);
        }
        if let Some(call_size) = self
            .calls
            .iter()
            .filter(|call| call.mechanism != CallMechanism::Arena)
            .map(|call| call.callee_upper_bound.saturating_add(16))
            .max()
        {
            size = align_up(size, 16)?;
            size = size
                .checked_add(call_size)
                .ok_or_else(|| "coroutine frame size overflowed".to_owned())?;
            max_align = max_align.max(16);
        }
        align_up(size, max_align)
    }

    pub(super) fn render_typedef(&self, frame_type: &str) -> Result<String, String> {
        if self.storage != FrameStorage::CoFrame {
            return Ok(String::new());
        }
        let mut out = String::from("typedef struct {\n    llg_co_frame_t co;\n");
        for field in &self.fields {
            out.push_str("    ");
            out.push_str(&declaration(&field.ty, &field.name));
            out.push_str(";\n");
        }
        let embedded = self
            .calls
            .iter()
            .filter(|call| call.mechanism != CallMechanism::Arena)
            .collect::<Vec<_>>();
        if !embedded.is_empty() {
            out.push_str("    union {\n");
            for call in embedded {
                match call.mechanism {
                    CallMechanism::Polled { .. } => {
                        out.push_str(&format!("        {} {};\n", call.callee_type, call.member))
                    }
                    CallMechanism::Anchored => out.push_str(&format!(
                        "        LLG_CO_ANCHORED({}) {};\n",
                        call.callee_type, call.member
                    )),
                    CallMechanism::Arena => unreachable!(),
                }
            }
            out.push_str("    } calls;\n");
        }
        out.push_str(&format!("}} {frame_type};\n"));
        Ok(out)
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
        "llg_string_t" => (32, 8),
        "sv4_select_plan_t" => (20, 4),
        "llg_event_t" => (8, 8),
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
    fn coframe_starts_with_header_and_rejects_duplicate_fields() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        assert_eq!(layout.declare("uint64_t", "ticks").unwrap(), "F->ticks");
        assert!(layout.declare("int", "ticks").is_err());
        let rendered = layout.render_typedef("p_frame_t").unwrap();
        assert!(rendered.starts_with("typedef struct {\n    llg_co_frame_t co;\n"));
    }

    #[test]
    fn call_union_uses_selected_mechanisms() {
        let mut layout = FrameLayout::new(FrameStorage::CoFrame);
        layout
            .add_call(1, "small_frame_t", CallMechanism::Polled { depth: 1 }, 64)
            .unwrap();
        layout
            .add_call(2, "deep_frame_t", CallMechanism::Anchored, 80)
            .unwrap();
        layout
            .add_call(3, "recursive_frame_t", CallMechanism::Arena, 96)
            .unwrap();
        let rendered = layout.render_typedef("caller_frame_t").unwrap();
        assert!(rendered.contains("small_frame_t c0;"));
        assert!(rendered.contains("LLG_CO_ANCHORED(deep_frame_t) a1;"));
        assert!(!rendered.contains("recursive_frame_t arena2"));
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
