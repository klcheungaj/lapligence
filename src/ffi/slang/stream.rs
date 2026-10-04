//! Receivers for the ABI v12 capture stream.
//!
//! # Design
//!
//! Slang elaborates and the C++ wrapper captures the design into its working
//! tables while the compilation is alive. Capture cannot emit a record the
//! moment it first meets a symbol: placeholder nodes are filled later, edges
//! change role or disappear, types patch their windows after recursion,
//! overridden parameters and source identities are marked at the end, and the
//! lexical pass reads the finished graph. Once capture is complete the wrapper
//! destroys the Slang compilation (and trims the glibc heap), then streams the
//! finished tables through the [`RawSink`] callback table into one
//! [`StreamBuilder`], releasing each native table (semantic nodes chunk by
//! chunk) as soon as it has been delivered. No native snapshot owner crosses
//! the boundary and no second copy of the native tables is made.
//!
//! ## Record catalogue and order
//!
//! `begin` delivers the counts of every table ([`RawStreamHeader`]); the
//! tables then follow in [`Table`] order, each as zero or more bounded batches,
//! and `end` closes the stream. Child tables precede the parents whose windows
//! index them (related diagnostics before diagnostics, value words before
//! constants, type ranges and members before types, parameters before
//! instances, edges before nodes, UDP rows before UDP tables), so every window
//! is checked against a complete table. Type, instance and semantic-node IDs
//! are their dense table indices; a forward reference (a member's type, a
//! node's parent or target, an edge's target) is therefore checked against
//! the announced count, and closing each table checks that exactly the
//! announced number of records arrived. Window ownership that can only be
//! known once a parent table is complete (every edge, range, member,
//! parameter and UDP row claimed exactly once) is checked when that table
//! closes. The builder never needs a second pass over native memory.
//!
//! ## Ownership and lifetime
//!
//! Records and their string views are borrowed only for the callback that
//! delivers them; receivers copy and validate what they keep (UTF-8 for text,
//! arbitrary bytes for string constants). The builder lives on the caller's
//! stack for one blocking `llg_slang_compile` call and is reached only through
//! the sink context pointer on the calling thread.
//!
//! ## Error flow
//!
//! Each receiver returns a `Result`; the trampoline records the first error,
//! converts a Rust panic into an internal error (no unwind crosses C), and
//! returns `LLG_SLANG_SINK_ABORT`. The wrapper then stops streaming, releases
//! all native state and returns `LLG_SLANG_STATUS_SINK_ABORTED`; [`compile`]
//! reports the recorded error. Native failures (C++ exceptions, budget
//! exhaustion during capture) never reach the sink and keep their status and
//! message. Count limits and the export byte budget are enforced from the
//! header before any table is reserved, and every string is charged as it
//! arrives, with the same diagnostics as the former snapshot decoder.
//!
//! [`compile`]: super::compile

use super::*;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub(super) const SINK_CONTINUE: u32 = 0;
pub(super) const SINK_ABORT: u32 = 1;

/// Streamed tables in delivery order; mirrors `LlgSlangStreamHeader`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Table {
    Files,
    RelatedDiagnostics,
    Diagnostics,
    ValueWords,
    Constants,
    TypeRanges,
    TypeMembers,
    Types,
    Parameters,
    Instances,
    SemanticEdges,
    SemanticNodes,
    UdpRows,
    UdpTables,
    LexicalTokens,
    SourceLibraries,
    LineDirectives,
}

const TABLE_COUNT: usize = Table::LineDirectives as usize + 1;
const TABLES: [Table; TABLE_COUNT] = [
    Table::Files,
    Table::RelatedDiagnostics,
    Table::Diagnostics,
    Table::ValueWords,
    Table::Constants,
    Table::TypeRanges,
    Table::TypeMembers,
    Table::Types,
    Table::Parameters,
    Table::Instances,
    Table::SemanticEdges,
    Table::SemanticNodes,
    Table::UdpRows,
    Table::UdpTables,
    Table::LexicalTokens,
    Table::SourceLibraries,
    Table::LineDirectives,
];

impl Table {
    fn label(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::RelatedDiagnostics => "related diagnostics",
            Self::Diagnostics => "diagnostics",
            Self::ValueWords => "constant value words",
            Self::Constants => "constants",
            Self::TypeRanges => "type ranges",
            Self::TypeMembers => "type members",
            Self::Types => "types",
            Self::Parameters => "parameters",
            Self::Instances => "instances",
            Self::SemanticEdges => "semantic edges",
            Self::SemanticNodes => "semantic nodes",
            Self::UdpRows => "UDP rows",
            Self::UdpTables => "UDP tables",
            Self::LexicalTokens => "lexical tokens",
            Self::SourceLibraries => "source library records",
            Self::LineDirectives => "line directive records",
        }
    }

    fn record_size(self) -> usize {
        match self {
            Self::Files => std::mem::size_of::<RawFile>(),
            Self::RelatedDiagnostics => std::mem::size_of::<RawRelatedDiagnostic>(),
            Self::Diagnostics => std::mem::size_of::<RawDiagnostic>(),
            Self::ValueWords => std::mem::size_of::<u64>(),
            Self::Constants => std::mem::size_of::<RawConstant>(),
            Self::TypeRanges => std::mem::size_of::<RawTypeRange>(),
            Self::TypeMembers => std::mem::size_of::<RawTypeMember>(),
            Self::Types => std::mem::size_of::<RawType>(),
            Self::Parameters => std::mem::size_of::<RawParameter>(),
            Self::Instances => std::mem::size_of::<RawInstance>(),
            Self::SemanticEdges => std::mem::size_of::<RawSemanticEdge>(),
            Self::SemanticNodes => std::mem::size_of::<RawSemanticNode>(),
            Self::UdpRows => std::mem::size_of::<RawUdpRow>(),
            Self::UdpTables => std::mem::size_of::<RawUdpTable>(),
            Self::LexicalTokens => std::mem::size_of::<RawLexicalToken>(),
            Self::SourceLibraries => std::mem::size_of::<RawSourceLibrary>(),
            Self::LineDirectives => std::mem::size_of::<RawLineDirective>(),
        }
    }
}

/// Announced counts and flags of one capture stream.
#[derive(Clone, Copy, Debug)]
struct Header {
    flags: u32,
    counts: [u64; TABLE_COUNT],
}

impl Header {
    fn count(&self, table: Table) -> u64 {
        self.counts[table as usize]
    }
}

/// Owned destination of one capture stream; see the module documentation.
pub(super) struct StreamBuilder {
    limits: Limits,
    /// Profile marker of the current phase: `native` until the header
    /// arrives, then `ffi.stream`.
    profile: Option<crate::profile::Stage>,
    header: Option<Header>,
    /// First table that may still deliver records; earlier tables are closed.
    open: usize,
    received: [u64; TABLE_COUNT],
    finished: bool,
    error: Option<SlangError>,
    output_bytes: u64,
    total_value_bits: u64,
    file_ids: HashSet<u64>,
    files: Vec<File>,
    related: Vec<RelatedDiagnostic>,
    diagnostics: Vec<Diagnostic>,
    value_words: Vec<u64>,
    constants: Vec<Constant>,
    type_ranges: Vec<TypeRange>,
    type_members: Vec<TypeMember>,
    claimed_ranges: Vec<bool>,
    claimed_members: Vec<bool>,
    types: Vec<Type>,
    parameters: Vec<Parameter>,
    claimed_parameters: Vec<bool>,
    instances: Vec<Instance>,
    semantic_edges: Vec<SemanticEdge>,
    claimed_edges: Vec<bool>,
    edge_keys: HashSet<(SemanticEdgeRole, u32)>,
    texts: TextInterner,
    semantic_nodes: Vec<SemanticNode>,
    udp_rows: Vec<PendingUdpRow>,
    claimed_rows: Vec<bool>,
    udp_table_ids: HashSet<u64>,
    udp_overlap: UdpOverlapValidator,
    udp_tables: Vec<UdpTable>,
    lexical_tokens: Vec<LexicalToken>,
    library_nodes: HashSet<u64>,
    source_libraries: Vec<SourceLibraryBinding>,
    line_directives: Vec<LineDirective>,
}

impl StreamBuilder {
    pub(super) fn new(limits: Limits) -> Self {
        Self {
            limits,
            profile: Some(crate::profile::Stage::new("native")),
            header: None,
            open: 0,
            received: [0; TABLE_COUNT],
            finished: false,
            error: None,
            output_bytes: 0,
            total_value_bits: 0,
            file_ids: HashSet::new(),
            files: Vec::new(),
            related: Vec::new(),
            diagnostics: Vec::new(),
            value_words: Vec::new(),
            constants: Vec::new(),
            type_ranges: Vec::new(),
            type_members: Vec::new(),
            claimed_ranges: Vec::new(),
            claimed_members: Vec::new(),
            types: Vec::new(),
            parameters: Vec::new(),
            claimed_parameters: Vec::new(),
            instances: Vec::new(),
            semantic_edges: Vec::new(),
            claimed_edges: Vec::new(),
            edge_keys: HashSet::new(),
            texts: TextInterner::default(),
            semantic_nodes: Vec::new(),
            udp_rows: Vec::new(),
            claimed_rows: Vec::new(),
            udp_table_ids: HashSet::new(),
            udp_overlap: UdpOverlapValidator::new(limits.max_semantic_edges),
            udp_tables: Vec::new(),
            lexical_tokens: Vec::new(),
            library_nodes: HashSet::new(),
            source_libraries: Vec::new(),
            line_directives: Vec::new(),
        }
    }

    /// The first error a receiver recorded, which aborted the stream.
    pub(super) fn take_error(&mut self) -> Option<SlangError> {
        self.error.take()
    }

    /// The owned snapshot of a completely delivered stream, and the profile
    /// marker of the stream phase, which the caller ends.
    pub(super) fn finish(self) -> Result<(Snapshot, Option<crate::profile::Stage>), SlangError> {
        if !self.finished {
            return Err(invalid_native(
                "capture stream ended without its end record",
            ));
        }
        let header = self
            .header
            .ok_or_else(|| invalid_native("capture stream has no header"))?;
        let snapshot = Snapshot {
            flags: header.flags,
            edition: LanguageEdition::from_snapshot_flags(header.flags)?,
            compilation_unit_mode: CompilationUnitMode::from_snapshot_flags(header.flags),
            files: self.files,
            diagnostics: self.diagnostics,
            instances: self.instances,
            parameters: self.parameters,
            types: self.types,
            constants: self.constants,
            semantic_nodes: self.semantic_nodes,
            semantic_edges: self.semantic_edges,
            lexical_tokens: self.lexical_tokens,
            type_ranges: self.type_ranges,
            type_members: self.type_members,
            udp_tables: self.udp_tables,
            source_libraries: self.source_libraries,
            line_directives: self.line_directives,
        };
        Ok((snapshot, self.profile))
    }

    fn header(&self) -> Result<&Header, SlangError> {
        self.header
            .as_ref()
            .ok_or_else(|| invalid_native("capture stream record arrived before its header"))
    }

    pub(super) fn begin(&mut self, raw: &RawStreamHeader) -> Result<(), SlangError> {
        if self.header.is_some() {
            return Err(invalid_native("capture stream repeated its header"));
        }
        if raw.abi_version != ABI_VERSION {
            return Err(invalid_native(format!(
                "Slang ABI version mismatch: expected {ABI_VERSION}, received {}",
                raw.abi_version
            )));
        }
        if raw.flags & !SNAPSHOT_KNOWN_FLAGS != 0 {
            return Err(invalid_native("snapshot contains unknown flags"));
        }
        let counts = [
            raw.file_count,
            raw.related_diagnostic_count,
            raw.diagnostic_count,
            raw.value_word_count,
            raw.constant_count,
            raw.type_range_count,
            raw.type_member_count,
            raw.type_count,
            raw.parameter_count,
            raw.instance_count,
            raw.semantic_edge_count,
            raw.semantic_node_count,
            raw.udp_row_count,
            raw.udp_table_count,
            raw.lexical_token_count,
            raw.source_library_count,
            raw.line_directive_count,
        ];
        let header = Header {
            flags: raw.flags,
            counts,
        };
        let limits = self.limits;
        enforce_count(header.count(Table::Files), limits.max_sources, "files")?;
        enforce_count(
            header.count(Table::Diagnostics),
            limits.max_diagnostics,
            "diagnostics",
        )?;
        enforce_count(
            header.count(Table::Instances),
            limits.max_instances,
            "instances",
        )?;
        enforce_count(
            header.count(Table::Parameters),
            limits.max_parameters,
            "parameters",
        )?;
        enforce_count(header.count(Table::Types), limits.max_types, "types")?;
        enforce_count(
            header.count(Table::SemanticNodes),
            limits.max_semantic_nodes,
            "semantic nodes",
        )?;
        enforce_count(
            header.count(Table::SemanticEdges),
            limits.max_semantic_edges,
            "semantic edges",
        )?;
        enforce_count(
            header.count(Table::LexicalTokens),
            limits.max_lexical_tokens,
            "lexical tokens",
        )?;
        enforce_count(
            header.count(Table::TypeRanges),
            limits.max_type_ranges,
            "type ranges",
        )?;
        enforce_count(
            header.count(Table::TypeMembers),
            limits.max_type_members,
            "type members",
        )?;
        enforce_count(
            header.count(Table::RelatedDiagnostics),
            limits.max_related_diagnostics,
            "related diagnostics",
        )?;
        // Each integer constant can require a partially filled word, so the
        // padded-word bound includes one extra word per constant and per plane.
        let max_words = limits
            .max_value_bits
            .div_ceil(64)
            .saturating_add(header.count(Table::Constants))
            .saturating_mul(2);
        enforce_count(
            header.count(Table::ValueWords),
            max_words,
            "constant value words",
        )?;
        enforce_count(
            header.count(Table::Constants),
            limits.max_constants,
            "constants",
        )?;
        // UDP declarations and rows are bounded by the same frontend record
        // limits as the semantic graph; their byte budget is checked below too.
        enforce_count(
            header.count(Table::UdpTables),
            limits.max_semantic_nodes,
            "UDP tables",
        )?;
        enforce_count(
            header.count(Table::UdpRows),
            limits.max_semantic_edges,
            "UDP rows",
        )?;
        // Each library record names one distinct semantic node.
        enforce_count(
            header.count(Table::SourceLibraries),
            limits.max_semantic_nodes,
            "source library records",
        )?;
        let mut output_bytes = 0_u64;
        for table in TABLES {
            let bytes = header
                .count(table)
                .checked_mul(table.record_size() as u64)
                .ok_or_else(|| invalid_native("native output record byte count overflowed"))?;
            output_bytes = output_bytes
                .checked_add(bytes)
                .ok_or_else(|| invalid_native("native output byte count overflowed"))?;
        }
        if output_bytes > limits.max_output_bytes {
            return Err(invalid_native(
                "native output records exceed the configured max_output_bytes",
            ));
        }
        self.output_bytes = output_bytes;
        // Every count is bounded by its limit above, so these reservations are
        // exactly the validated record counts.
        let capacity = |table: Table| usize::try_from(header.count(table)).unwrap_or(0);
        self.files.reserve_exact(capacity(Table::Files));
        self.related
            .reserve_exact(capacity(Table::RelatedDiagnostics));
        self.diagnostics.reserve_exact(capacity(Table::Diagnostics));
        self.value_words.reserve_exact(capacity(Table::ValueWords));
        self.constants.reserve_exact(capacity(Table::Constants));
        self.type_ranges.reserve_exact(capacity(Table::TypeRanges));
        self.type_members
            .reserve_exact(capacity(Table::TypeMembers));
        self.types.reserve_exact(capacity(Table::Types));
        self.parameters.reserve_exact(capacity(Table::Parameters));
        self.instances.reserve_exact(capacity(Table::Instances));
        self.semantic_edges
            .reserve_exact(capacity(Table::SemanticEdges));
        self.semantic_nodes
            .reserve_exact(capacity(Table::SemanticNodes));
        self.udp_rows.reserve_exact(capacity(Table::UdpRows));
        self.udp_tables.reserve_exact(capacity(Table::UdpTables));
        self.lexical_tokens
            .reserve_exact(capacity(Table::LexicalTokens));
        self.source_libraries
            .reserve_exact(capacity(Table::SourceLibraries));
        self.line_directives
            .reserve_exact(capacity(Table::LineDirectives));
        self.header = Some(header);
        // Native capture is complete once the header arrives; the stream
        // phase covers delivery and decoding.
        self.profile = None;
        self.profile = Some(crate::profile::Stage::new("ffi.stream"));
        Ok(())
    }

    pub(super) fn end(&mut self) -> Result<(), SlangError> {
        self.header()?;
        if self.finished {
            return Err(invalid_native("capture stream repeated its end record"));
        }
        while self.open < TABLE_COUNT {
            self.close(TABLES[self.open])?;
            self.open += 1;
        }
        self.finished = true;
        Ok(())
    }

    /// Admit one batch of `table`, closing every earlier table first.
    fn enter(&mut self, table: Table, count: usize) -> Result<(), SlangError> {
        let announced = self.header()?.count(table);
        if self.finished {
            return Err(invalid_native(
                "capture stream delivered records after its end",
            ));
        }
        if (table as usize) < self.open {
            return Err(invalid_native(format!(
                "capture stream delivered {} out of order",
                table.label()
            )));
        }
        while self.open < table as usize {
            self.close(TABLES[self.open])?;
            self.open += 1;
        }
        let received = &mut self.received[table as usize];
        *received = received
            .checked_add(count as u64)
            .filter(|total| *total <= announced)
            .ok_or_else(|| {
                invalid_native(format!(
                    "capture stream delivered more {} than announced",
                    table.label()
                ))
            })?;
        Ok(())
    }

    /// Check that `table` delivered its announced records and run the checks
    /// that need the complete table.
    fn close(&mut self, table: Table) -> Result<(), SlangError> {
        if self.received[table as usize] != self.header()?.count(table) {
            return Err(invalid_native(format!(
                "capture stream ended {} early",
                table.label()
            )));
        }
        match table {
            // Diagnostics own copies of their related records.
            Table::Diagnostics => self.related = Vec::new(),
            // Constants own copies of their words.
            Table::Constants => self.value_words = Vec::new(),
            Table::Types => {
                if self.claimed_ranges.iter().any(|claimed| !claimed) {
                    return Err(invalid_native("type range is not owned by a type"));
                }
                if self.claimed_members.iter().any(|claimed| !claimed) {
                    return Err(invalid_native("type member is not owned by a type"));
                }
                self.claimed_ranges = Vec::new();
                self.claimed_members = Vec::new();
            }
            Table::Instances => {
                if self.claimed_parameters.iter().any(|claimed| !claimed) {
                    return Err(invalid_native(
                        "parameter record is not covered by its owner instance window",
                    ));
                }
                self.claimed_parameters = Vec::new();
            }
            Table::SemanticNodes => {
                if self.claimed_edges.iter().any(|claimed| !claimed) {
                    return Err(invalid_native("semantic edge is not owned by a node"));
                }
                self.claimed_edges = Vec::new();
                self.edge_keys = HashSet::new();
                self.texts = TextInterner::default();
            }
            Table::UdpTables => {
                if self.claimed_rows.iter().any(|claimed| !claimed) {
                    return Err(invalid_native("UDP row is not owned by a table"));
                }
                self.claimed_rows = Vec::new();
                self.udp_rows = Vec::new();
            }
            Table::LineDirectives => {
                self.line_directives =
                    finish_line_directives(std::mem::take(&mut self.line_directives))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn charge(&mut self, raw: RawString) -> Result<(), SlangError> {
        charge_output_string(&mut self.output_bytes, raw, self.limits.max_output_bytes)
    }

    pub(super) fn files(&mut self, raw: &[RawFile]) -> Result<(), SlangError> {
        self.enter(Table::Files, raw.len())?;
        for item in raw {
            self.charge(item.name)?;
            if item.id == INVALID_ID || !self.file_ids.insert(item.id) {
                return Err(invalid_native(
                    "snapshot contains an invalid or duplicate file id",
                ));
            }
            if item.byte_len > self.limits.max_source_bytes {
                return Err(invalid_native(
                    "file byte length exceeds the configured limit",
                ));
            }
            self.files.push(File {
                id: item.id,
                // SAFETY: stream records and their strings are valid for the
                // callback that delivered them.
                name: unsafe { copy_string(item.name, "file name")? },
                byte_len: item.byte_len,
                text: String::new(),
            });
        }
        Ok(())
    }

    pub(super) fn related_diagnostics(
        &mut self,
        raw: &[RawRelatedDiagnostic],
    ) -> Result<(), SlangError> {
        self.enter(Table::RelatedDiagnostics, raw.len())?;
        for item in raw {
            self.charge(item.message)?;
            let related = decode_related(item, &self.files)?;
            self.related.push(related);
        }
        Ok(())
    }

    pub(super) fn diagnostics(&mut self, raw: &[RawDiagnostic]) -> Result<(), SlangError> {
        self.enter(Table::Diagnostics, raw.len())?;
        for item in raw {
            for value in [item.name, item.option_name, item.message] {
                self.charge(value)?;
            }
            let diagnostic = decode_diagnostic(item, &self.related, &self.files)?;
            self.diagnostics.push(diagnostic);
        }
        Ok(())
    }

    pub(super) fn value_words(&mut self, raw: &[u64]) -> Result<(), SlangError> {
        self.enter(Table::ValueWords, raw.len())?;
        self.value_words.extend_from_slice(raw);
        Ok(())
    }

    pub(super) fn constants(&mut self, raw: &[RawConstant]) -> Result<(), SlangError> {
        self.enter(Table::Constants, raw.len())?;
        for item in raw {
            self.charge(item.text)?;
            let constant = decode_constant(
                item,
                &self.value_words,
                &self.limits,
                &mut self.total_value_bits,
            )?;
            self.constants.push(constant);
        }
        Ok(())
    }

    pub(super) fn type_ranges(&mut self, raw: &[RawTypeRange]) -> Result<(), SlangError> {
        self.enter(Table::TypeRanges, raw.len())?;
        for item in raw {
            self.type_ranges.push(decode_type_range(item)?);
            self.claimed_ranges.push(false);
        }
        Ok(())
    }

    pub(super) fn type_members(&mut self, raw: &[RawTypeMember]) -> Result<(), SlangError> {
        self.enter(Table::TypeMembers, raw.len())?;
        let type_count = self.header()?.count(Table::Types);
        for item in raw {
            self.charge(item.name)?;
            let member = decode_type_member(item, type_count, self.constants.len())?;
            self.type_members.push(member);
            self.claimed_members.push(false);
        }
        Ok(())
    }

    pub(super) fn types(&mut self, raw: &[RawType]) -> Result<(), SlangError> {
        self.enter(Table::Types, raw.len())?;
        let type_count = self.header()?.count(Table::Types);
        for item in raw {
            self.charge(item.display_name)?;
            let ty = decode_type(
                item,
                self.types.len() as u64,
                type_count,
                &mut self.claimed_ranges,
                &mut self.claimed_members,
            )?;
            self.types.push(ty);
        }
        Ok(())
    }

    pub(super) fn parameters(&mut self, raw: &[RawParameter]) -> Result<(), SlangError> {
        self.enter(Table::Parameters, raw.len())?;
        let instance_count = self.header()?.count(Table::Instances);
        for item in raw {
            self.charge(item.name)?;
            let parameter = decode_parameter(
                item,
                &self.files,
                instance_count,
                self.types.len(),
                self.constants.len(),
            )?;
            self.parameters.push(parameter);
            self.claimed_parameters.push(false);
        }
        Ok(())
    }

    pub(super) fn instances(&mut self, raw: &[RawInstance]) -> Result<(), SlangError> {
        self.enter(Table::Instances, raw.len())?;
        let instance_count = self.header()?.count(Table::Instances);
        for item in raw {
            for value in [item.name, item.definition_name] {
                self.charge(value)?;
            }
            let instance = decode_instance(
                item,
                self.instances.len() as u64,
                instance_count,
                &self.files,
                &self.parameters,
                &mut self.claimed_parameters,
            )?;
            self.instances.push(instance);
        }
        Ok(())
    }

    pub(super) fn semantic_edges(&mut self, raw: &[RawSemanticEdge]) -> Result<(), SlangError> {
        self.enter(Table::SemanticEdges, raw.len())?;
        let node_count = self.header()?.count(Table::SemanticNodes);
        for item in raw {
            self.semantic_edges
                .push(decode_semantic_edge(item, node_count)?);
            self.claimed_edges.push(false);
        }
        Ok(())
    }

    pub(super) fn semantic_nodes(&mut self, raw: &[RawSemanticNode]) -> Result<(), SlangError> {
        self.enter(Table::SemanticNodes, raw.len())?;
        let node_count = self.header()?.count(Table::SemanticNodes);
        for item in raw {
            for value in [item.name, item.detail, item.definition_name] {
                self.charge(value)?;
            }
            let node = decode_semantic_node(
                item,
                self.semantic_nodes.len() as u64,
                SemanticNodeContext {
                    node_count,
                    edges: &self.semantic_edges,
                    claimed_edges: &mut self.claimed_edges,
                    edge_keys: &mut self.edge_keys,
                    files: &self.files,
                    type_count: self.types.len(),
                    constant_count: self.constants.len(),
                    texts: &mut self.texts,
                },
            )?;
            self.semantic_nodes.push(node);
        }
        Ok(())
    }

    pub(super) fn udp_rows(&mut self, raw: &[RawUdpRow]) -> Result<(), SlangError> {
        self.enter(Table::UdpRows, raw.len())?;
        for item in raw {
            self.charge(item.inputs)?;
            self.udp_rows.push(decode_udp_row(item)?);
            self.claimed_rows.push(false);
        }
        Ok(())
    }

    pub(super) fn udp_tables(&mut self, raw: &[RawUdpTable]) -> Result<(), SlangError> {
        self.enter(Table::UdpTables, raw.len())?;
        for item in raw {
            self.charge(item.name)?;
            let table = decode_udp_table(
                item,
                UdpTableContext {
                    rows: &mut self.udp_rows,
                    claimed_rows: &mut self.claimed_rows,
                    table_ids: &mut self.udp_table_ids,
                    overlap: &mut self.udp_overlap,
                    semantic_nodes: &self.semantic_nodes,
                },
            )?;
            self.udp_tables.push(table);
        }
        Ok(())
    }

    pub(super) fn lexical_tokens(&mut self, raw: &[RawLexicalToken]) -> Result<(), SlangError> {
        self.enter(Table::LexicalTokens, raw.len())?;
        for item in raw {
            self.charge(item.text)?;
            let token = decode_lexical_token(item, &self.files, self.semantic_nodes.len())?;
            self.lexical_tokens.push(token);
        }
        Ok(())
    }

    pub(super) fn source_libraries(&mut self, raw: &[RawSourceLibrary]) -> Result<(), SlangError> {
        self.enter(Table::SourceLibraries, raw.len())?;
        for item in raw {
            self.charge(item.library)?;
            let binding =
                decode_source_library(item, self.semantic_nodes.len(), &mut self.library_nodes)?;
            self.source_libraries.push(binding);
        }
        Ok(())
    }

    pub(super) fn line_directives(&mut self, raw: &[RawLineDirective]) -> Result<(), SlangError> {
        self.enter(Table::LineDirectives, raw.len())?;
        for item in raw {
            self.charge(item.logical_file)?;
            let directive = decode_line_directive(item, &self.files)?;
            self.line_directives.push(directive);
        }
        Ok(())
    }
}

/// Run one receiver for a sink callback, recording its first error.
///
/// # Safety
/// `context` must be null or the [`StreamBuilder`] installed in the sink of
/// the blocking `llg_slang_compile` call that invokes this callback, with no
/// other live reference to it.
pub(super) unsafe fn receive(
    context: *mut c_void,
    step: impl FnOnce(&mut StreamBuilder) -> Result<(), SlangError>,
) -> u32 {
    if context.is_null() {
        return SINK_ABORT;
    }
    // SAFETY: the caller guarantees `context` is the exclusive builder of the
    // current compile call; the bridge invokes callbacks synchronously on the
    // calling thread, so no other reference is live during this call.
    let builder = unsafe { &mut *context.cast::<StreamBuilder>() };
    if builder.error.is_some() {
        return SINK_ABORT;
    }
    // No unwind may cross the C callback boundary; a panic becomes the
    // stream's recorded internal error.
    let outcome = catch_unwind(AssertUnwindSafe(|| step(&mut *builder)));
    let error = match outcome {
        Ok(Ok(())) => return SINK_CONTINUE,
        Ok(Err(error)) => error,
        Err(_) => SlangError::new(SlangErrorKind::Internal, "capture stream receiver panicked"),
    };
    builder.error = Some(error);
    SINK_ABORT
}

/// Deliver one borrowed batch of `table` records to `step`.
///
/// # Safety
/// As for [`receive`]; in addition, for nonzero `count`, `records` must
/// reference `count` aligned initialized records whose string views remain
/// valid until this call returns.
unsafe fn receive_batch<T>(
    context: *mut c_void,
    records: *const T,
    count: u64,
    table: Table,
    step: fn(&mut StreamBuilder, &[T]) -> Result<(), SlangError>,
) -> u32 {
    // SAFETY: the caller upholds `receive`'s context contract.
    unsafe {
        receive(context, |builder| {
            // SAFETY: the caller guarantees the batch pointer and length; the
            // slice does not outlive this callback.
            let records = foreign_slice(records, count, table.label())?;
            step(builder, records)
        })
    }
}

unsafe extern "C" fn sink_begin(context: *mut c_void, header: *const RawStreamHeader) -> u32 {
    // SAFETY: the bridge passes the sink's builder and a header valid for
    // this call; a null header is rejected before it is read.
    unsafe {
        receive(context, |builder| {
            // SAFETY: a non-null header points to an initialized record that
            // outlives this callback.
            let header = header
                .as_ref()
                .ok_or_else(|| invalid_native("capture stream header is null"))?;
            builder.begin(header)
        })
    }
}

unsafe extern "C" fn sink_end(context: *mut c_void) -> u32 {
    // SAFETY: the bridge passes the sink's builder.
    unsafe { receive(context, StreamBuilder::end) }
}

/// Define one `extern "C"` batch receiver per streamed table.
macro_rules! batch_receivers {
    ($($callback:ident: $raw:ty => $table:ident, $method:ident;)*) => {
        $(
            unsafe extern "C" fn $callback(
                context: *mut c_void,
                records: *const $raw,
                count: u64,
            ) -> u32 {
                // SAFETY: the bridge passes the sink's builder and a batch of
                // `count` records that remain valid for this call.
                unsafe {
                    receive_batch(context, records, count, Table::$table, StreamBuilder::$method)
                }
            }
        )*
    };
}

batch_receivers! {
    sink_files: RawFile => Files, files;
    sink_related_diagnostics: RawRelatedDiagnostic => RelatedDiagnostics, related_diagnostics;
    sink_diagnostics: RawDiagnostic => Diagnostics, diagnostics;
    sink_value_words: u64 => ValueWords, value_words;
    sink_constants: RawConstant => Constants, constants;
    sink_type_ranges: RawTypeRange => TypeRanges, type_ranges;
    sink_type_members: RawTypeMember => TypeMembers, type_members;
    sink_types: RawType => Types, types;
    sink_parameters: RawParameter => Parameters, parameters;
    sink_instances: RawInstance => Instances, instances;
    sink_semantic_edges: RawSemanticEdge => SemanticEdges, semantic_edges;
    sink_semantic_nodes: RawSemanticNode => SemanticNodes, semantic_nodes;
    sink_udp_rows: RawUdpRow => UdpRows, udp_rows;
    sink_udp_tables: RawUdpTable => UdpTables, udp_tables;
    sink_lexical_tokens: RawLexicalToken => LexicalTokens, lexical_tokens;
    sink_source_libraries: RawSourceLibrary => SourceLibraries, source_libraries;
    sink_line_directives: RawLineDirective => LineDirectives, line_directives;
}

/// The callback table whose context is `builder`. The returned value borrows
/// `builder` through a raw pointer: it must only be passed to one blocking
/// compile call while `builder` is otherwise unused.
pub(super) fn sink_for(builder: &mut StreamBuilder) -> RawSink {
    RawSink {
        context: (builder as *mut StreamBuilder).cast(),
        begin: sink_begin,
        files: sink_files,
        related_diagnostics: sink_related_diagnostics,
        diagnostics: sink_diagnostics,
        value_words: sink_value_words,
        constants: sink_constants,
        type_ranges: sink_type_ranges,
        type_members: sink_type_members,
        types: sink_types,
        parameters: sink_parameters,
        instances: sink_instances,
        semantic_edges: sink_semantic_edges,
        semantic_nodes: sink_semantic_nodes,
        udp_rows: sink_udp_rows,
        udp_tables: sink_udp_tables,
        lexical_tokens: sink_lexical_tokens,
        source_libraries: sink_source_libraries,
        line_directives: sink_line_directives,
        end: sink_end,
    }
}
