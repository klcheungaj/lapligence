//! Packed boolean attributes of an owned semantic node.
//!
//! A capture holds one node per semantic construct (600k at 10k processes),
//! and the node used to carry its 33 booleans as separate bytes. They are now
//! bits of one [`SemanticFlags`] word; [`SemanticNode`] exposes each under its
//! original name as a `bool` accessor, so consumers read `node.is_bad()`.

use super::{SemanticNode, ARGUMENT_CONST_REF, ARGUMENT_REF_STATIC};

/// Bit set of the boolean attributes of one semantic node.
///
/// Bits 0..=12 and 16..=31 reuse the position the ABI gives the same attribute
/// in the raw node `flags` word, so decoding is a mask. Raw bits 13..=15 are
/// the definition-kind tags, which the owned node keeps as an enum instead.
/// Bits 32 and 33 are derived from the argument metadata.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SemanticFlags(u64);

/// Raw node flag bits that map one to one onto owned flag bits.
const RAW_FLAG_MASK: u32 = !(0b111 << 13);

macro_rules! semantic_flags {
    ($($(#[$doc:meta])* $accessor:ident = $constant:ident @ $bit:expr;)*) => {
        impl SemanticFlags {
            $($(#[$doc])* pub const $constant: SemanticFlags = SemanticFlags(1 << $bit);)*

            /// Every defined flag with its accessor's name and function.
            pub(super) const ALL: &'static [(&'static str, SemanticFlags, fn(&SemanticNode) -> bool)] =
                &[$((stringify!($accessor), SemanticFlags::$constant, SemanticNode::$accessor)),*];
        }

        impl SemanticNode {
            $($(#[$doc])*
            #[inline]
            pub fn $accessor(&self) -> bool {
                self.flags.contains(SemanticFlags::$constant)
            })*
        }
    };
}

semantic_flags! {
    is_bad = IS_BAD @ 0;
    is_uninstantiated = IS_UNINSTANTIATED @ 1;
    is_automatic = IS_AUTOMATIC @ 2;
    is_static = IS_STATIC @ 3;
    is_top = IS_TOP @ 4;
    is_implicit = IS_IMPLICIT @ 5;
    is_local = IS_LOCAL @ 6;
    is_nonblocking = IS_NONBLOCKING @ 7;
    is_input = IS_INPUT @ 8;
    is_output = IS_OUTPUT @ 9;
    is_inout = IS_INOUT @ 10;
    is_ref = IS_REF @ 11;
    is_implicit_conversion = IS_IMPLICIT_CONVERSION @ 12;
    is_indexed_up = IS_INDEXED_UP @ 16;
    is_indexed_down = IS_INDEXED_DOWN @ 17;
    case_wildcard_x_or_z = CASE_WILDCARD_X_OR_Z @ 18;
    case_wildcard_z = CASE_WILDCARD_Z @ 19;
    case_inside = CASE_INSIDE @ 20;
    is_posedge = IS_POSEDGE @ 21;
    is_negedge = IS_NEGEDGE @ 22;
    is_both_edges = IS_BOTH_EDGES @ 23;
    is_primitive_declaration = IS_PRIMITIVE_DECLARATION @ 24;
    is_primitive_instance = IS_PRIMITIVE_INSTANCE @ 25;
    is_primitive_port = IS_PRIMITIVE_PORT @ 26;
    is_task = IS_TASK @ 27;
    port_connection_present = PORT_CONNECTION_PRESENT @ 28;
    port_connection_open = PORT_CONNECTION_OPEN @ 29;
    is_propagated_conversion = IS_PROPAGATED_CONVERSION @ 30;
    method_with_clause = METHOD_WITH_CLAUSE @ 31;
    /// `const ref` qualification for a formal argument.
    is_const_ref = IS_CONST_REF @ 32;
    /// `ref static` qualification for a formal argument.
    is_ref_static = IS_REF_STATIC @ 33;
}

impl SemanticFlags {
    pub const EMPTY: SemanticFlags = SemanticFlags(0);

    /// Whether every bit of `flag` is set.
    #[inline]
    pub const fn contains(self, flag: SemanticFlags) -> bool {
        self.0 & flag.0 == flag.0
    }

    /// The raw bit pattern, for stable ordering and debugging.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Flags of a decoded node: the raw ABI word plus the argument qualifiers
    /// that Slang reports through `auxiliary` for argument nodes only.
    pub(super) fn from_raw(raw_flags: u32, is_argument: bool, auxiliary: u64) -> SemanticFlags {
        let mut bits = u64::from(raw_flags & RAW_FLAG_MASK);
        if is_argument {
            if auxiliary & ARGUMENT_CONST_REF != 0 {
                bits |= Self::IS_CONST_REF.0;
            }
            if auxiliary & ARGUMENT_REF_STATIC != 0 {
                bits |= Self::IS_REF_STATIC.0;
            }
        }
        SemanticFlags(bits)
    }
}

impl std::ops::BitOr for SemanticFlags {
    type Output = SemanticFlags;
    fn bitor(self, other: SemanticFlags) -> SemanticFlags {
        SemanticFlags(self.0 | other.0)
    }
}

impl std::fmt::Debug for SemanticFlags {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut set = formatter.debug_set();
        for (name, flag, _) in Self::ALL {
            if self.contains(*flag) {
                set.entry(&format_args!("{name}"));
            }
        }
        set.finish()
    }
}
