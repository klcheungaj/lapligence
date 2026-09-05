//! Owned declaration/reference indexing and binding resolution.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct SymEntry {
    pub name: String,
    pub kind: SymKind,
    /// Absolute source file path.
    pub file: String,
    /// 0-based line of the occurrence.
    pub line: u32,
    /// 0-based column of the occurrence.
    pub col: u32,
    /// 0-based end line (usually `== line`).
    pub end_line: u32,
    /// 0-based end column (exclusive-ish; `col + name length`).
    pub end_col: u32,
    /// `true` for declaration sites, `false` for reference sites.
    pub is_decl: bool,
    /// Enclosing scope name (module/package/instance full name) used to
    /// disambiguate same-named objects; best-effort for references.
    pub scope: Option<String>,
    /// Hover text for the object (SystemVerilog declaration snippet).
    pub detail: Option<String>,
}

/// Workspace-wide symbol index: every declaration and reference site the
/// pipeline knows about, with lookup structures for position/name queries.
///
/// Built by [`SymbolIndex::build`] (or `Analysis::new`) from the design model
/// and the token lists; never touches Surelog afterwards.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// All declaration sites, in build order.
    pub decls: Vec<SymEntry>,
    /// All reference sites, in build order.
    pub refs: Vec<SymEntry>,
    /// Declarations per file, sorted by (line, col).
    decls_by_file: HashMap<String, Vec<SymEntry>>,
    /// References per file, sorted by (line, col).
    refs_by_file: HashMap<String, Vec<SymEntry>>,
    /// Name → indices into `decls` (exact, case-sensitive match).
    decls_by_name: HashMap<String, Vec<usize>>,
    /// Decl index → clean module definition name (for `Instance` decls).
    pub(super) instance_def: HashMap<usize, String>,
    /// Named port connection labels (`.clk` in `m u0(.clk(c))`), keyed by
    /// label position → index into `decls` of the child module's port
    /// declaration.
    pub(super) port_labels: HashMap<(String, u32, u32), usize>,
    /// Named parameter override labels (the `W` in `m u0 #(.W(4)) (...)`),
    /// keyed by label position → index into `decls` of the CHILD module's
    /// parameter declaration.
    pub(super) param_labels: HashMap<(String, u32, u32), usize>,
    /// Positions of scanned named PARAMETER override labels that did NOT
    /// resolve to a child-module parameter.  A definition request at such a
    /// position must return NO result rather than falling through to the
    /// name-based rules, which would jump to a same-named object of the
    /// instantiating scope (a `localparam W` decoy) or an arbitrary
    /// workspace match.
    unresolved_param_labels: HashSet<(String, u32, u32)>,
    /// Parse-backed enum uses for which no unique declaration was found.
    /// These positions must not fall through to the broad name-based resolver.
    unresolved_enum_refs: HashSet<(String, u32, u32)>,
}

/// VPI types that always denote a reference site (from `walk_expr` in
/// `core::tokens`; verified empirically that refs report `vpiRefObj`).
pub(super) const REF_TOKEN_TYPES: &[i32] = &[
    llg::ffi::vpi::vpiRefObj,
    llg::ffi::vpi::uhdmref_obj,
    llg::ffi::vpi::uhdmref_var,
    llg::ffi::vpi::vpiVarSelect,
];

/// VPI types that denote a net/var/port/parameter object — ambiguous between
/// declaration and reference sites (see the classification in
/// [`SymbolIndex::from_parts`]).
pub(super) const SIGNAL_DECL_TYPES: &[i32] = &[
    llg::ffi::vpi::vpiNet,
    llg::ffi::vpi::vpiNetBit,
    llg::ffi::vpi::vpiReg,
    llg::ffi::vpi::vpiRegBit,
    llg::ffi::vpi::vpiPort,
    llg::ffi::vpi::vpiPortBit,
    llg::ffi::vpi::vpiLogicVar,
    llg::ffi::vpi::vpiIntegerVar,
    llg::ffi::vpi::vpiRealVar,
    llg::ffi::vpi::vpiTimeVar,
    llg::ffi::vpi::uhdmlogic_var,
    llg::ffi::vpi::uhdmnet,
    llg::ffi::vpi::uhdmlogic_net,
    llg::ffi::vpi::uhdmint_var,
    llg::ffi::vpi::uhdmreal_var,
    llg::ffi::vpi::uhdmbit_var,
    llg::ffi::vpi::uhdmbyte_var,
    llg::ffi::vpi::uhdmshort_int_var,
    llg::ffi::vpi::uhdmlong_int_var,
    llg::ffi::vpi::vpiParameter,
    llg::ffi::vpi::vpiSpecParam,
    llg::ffi::vpi::uhdmparameter,
];

/// VPI types that denote a module/interface instance name at its
/// instantiation site (parse-tree classified; see `paName_of_instance`).
pub(super) const INSTANCE_NAME_TOKEN_TYPES: &[i32] = &[
    llg::ffi::vpi::uhdmlogic_var,
    llg::ffi::vpi::uhdmmodule_inst,
    llg::ffi::vpi::uhdminterface_inst,
];

/// Maximum distance (0-based lines) between an instance declaration and a
/// named port-connection label on a later line for the multi-line heuristic in
/// [`port_label_candidate`].  Labels farther below their instance than this are
/// not associated with it (they are more likely to belong to a different
/// instantiation or to be unrelated `vpiFunction`/`vpiTask` tokens).
pub(super) const PORT_LABEL_MAX_SPAN: u32 = 50;

impl SymbolIndex {
    /// Build the index from an [`Analysis`].
    ///
    /// `Analysis::new` builds the index internally via [`SymbolIndex::from_parts`];
    /// this entry point is part of the public API for external consumers.
    #[allow(dead_code)]
    pub fn build(a: &Analysis) -> SymbolIndex {
        SymbolIndex::from_parts(
            &a.model,
            &a.tokens,
            None,
            &[],
            &[],
            &HashSet::new(),
            &HashSet::new(),
        )
    }

    /// Merge indexes produced by independent analysis passes.
    ///
    /// Input order is preserved, and duplicate occurrences at the same
    /// position are kept from the first index.  The lookup maps are rebuilt
    /// from the merged vectors so declaration indices remain deterministic.
    /// Metadata whose values refer to declaration indices (`instance_def`,
    /// `port_labels` and `param_labels`) is remapped as entries are appended;
    /// unresolved-label positions union as-is.
    pub fn merge<I, T>(indices: I) -> SymbolIndex
    where
        I: IntoIterator<Item = T>,
        T: Borrow<SymbolIndex>,
    {
        let mut merged = SymbolIndex::default();
        for index in indices {
            merged.merge_one(index.borrow());
        }
        merged.rebuild_lookup_maps();
        merged
    }

    fn merge_one(&mut self, other: &SymbolIndex) {
        let mut decl_positions: HashMap<(String, u32, u32), usize> = self
            .decls
            .iter()
            .enumerate()
            .map(|(idx, d)| ((d.file.clone(), d.line, d.col), idx))
            .collect();
        let mut decl_remap = Vec::with_capacity(other.decls.len());
        for decl in &other.decls {
            let key = (decl.file.clone(), decl.line, decl.col);
            let new_idx = if let Some(&idx) = decl_positions.get(&key) {
                idx
            } else {
                let idx = self.decls.len();
                self.decls.push(decl.clone());
                decl_positions.insert(key, idx);
                idx
            };
            decl_remap.push(new_idx);
        }

        let mut ref_positions: HashSet<(String, u32, u32)> = self
            .refs
            .iter()
            .map(|r| (r.file.clone(), r.line, r.col))
            .collect();
        for reference in &other.refs {
            let key = (reference.file.clone(), reference.line, reference.col);
            if ref_positions.insert(key) {
                self.refs.push(reference.clone());
            }
        }

        for (&old_idx, def_name) in &other.instance_def {
            if let Some(&new_idx) = decl_remap.get(old_idx) {
                self.instance_def
                    .entry(new_idx)
                    .or_insert_with(|| def_name.clone());
            }
        }
        for (key, &old_idx) in &other.port_labels {
            if let Some(&new_idx) = decl_remap.get(old_idx) {
                self.port_labels.entry(key.clone()).or_insert(new_idx);
            }
        }
        for (key, &old_idx) in &other.param_labels {
            if let Some(&new_idx) = decl_remap.get(old_idx) {
                self.param_labels.entry(key.clone()).or_insert(new_idx);
            }
        }
        self.unresolved_param_labels
            .extend(other.unresolved_param_labels.iter().cloned());
        self.unresolved_enum_refs
            .extend(other.unresolved_enum_refs.iter().cloned());
    }

    pub(super) fn rebuild_lookup_maps(&mut self) {
        self.decls_by_file.clear();
        self.refs_by_file.clear();
        self.decls_by_name.clear();

        for (idx, decl) in self.decls.iter().enumerate() {
            self.decls_by_name
                .entry(decl.name.clone())
                .or_default()
                .push(idx);
            self.decls_by_file
                .entry(decl.file.clone())
                .or_default()
                .push(decl.clone());
        }
        for reference in &self.refs {
            self.refs_by_file
                .entry(reference.file.clone())
                .or_default()
                .push(reference.clone());
        }
        for entries in self.decls_by_file.values_mut() {
            entries.sort_by_key(|entry| (entry.line, entry.col));
        }
        for entries in self.refs_by_file.values_mut() {
            entries.sort_by_key(|entry| (entry.line, entry.col));
        }
    }

    /// Build the index from the owned model + token parts.
    ///
    /// # Declarations
    ///
    /// * modules and packages from the model (position refined to the *name*
    ///   token in the declaration file when available),
    /// * one entry per instance at its instantiation site,
    /// * functions/tasks from the model (per-instance clones with the
    ///   definition file/position and a signature detail),
    /// * ports / nets / vars / parameters from tokens whose position is a
    ///   declaration site (see below).
    ///
    /// # References
    ///
    /// * token nodes with a reference VPI type (`vpiRefObj`, `uhdmref_obj`,
    ///   `uhdmref_var`, `vpiVarSelect`) or a connection-label synthetic type
    ///   (`TOKEN_PORT_CONN_LABEL`, `TOKEN_PARAM_CONN_LABEL`),
    /// * module type names at instantiation sites (`uhdmclass_defn` tokens
    ///   whose name matches a module definition),
    /// * named port connections (`vpiFunction`/`vpiTask` tokens whose name is
    ///   a known port/signal/parameter, plus every `TOKEN_PORT_CONN_LABEL`
    ///   token); their child port declaration is precomputed into
    ///   `port_labels` (see [`port_label_candidate`] and
    ///   [`resolve_port_label`]),
    /// * named parameter overrides (classifier-labeled `TOKEN_PARAM_CONN_LABEL`
    ///   tokens at scanned `paNamed_parameter_assignment` positions — `pairs`
    ///   supplies both the positions and each instantiation's module type, so
    ///   no positional guessing is involved); their child parameter declaration
    ///   is precomputed into `param_labels` (see [`resolve_param_override`]),
    ///   while unresolvable label positions land in `unresolved_param_labels`.
    ///
    /// # Decl vs. ref for ambiguous signal tokens
    ///
    /// Empirically (Surelog v1.86 + UHDM): a true declaration site carries at
    /// least two tokens (VPI walker + parse tree) at the same position and no
    /// `vpiRefObj` companion, while reference sites have a `vpiRefObj`
    /// companion or a single token.  Port declarations additionally emit the
    /// direction-specific `TOKEN_PORT_*` types, which appear only at
    /// declaration sites.
    /// `parse_decls` carries the parse-tree declaration positions recorded by
    /// [`collect_parse_tokens`].  It is `Some` ONLY for the syntax-broken
    /// fallback path: without instances, `signal_names` would stay empty and
    /// every parse-classified port/net/var token would be dropped by
    /// [`classify_token`]'s gate; seeded sets plus per-position
    /// `forced_decl` restore them.  The elaborated pipeline passes `None` so
    /// UHDM multi-view classification is untouched.
    pub(super) fn from_parts(
        model: &DesignModel,
        tokens: &[FileTokens],
        parse_decls: Option<&ParseDeclPositions>,
        pairs: &[NamedPortConn],
        parse_enum_decls: &[ParseEnumDecl],
        parse_enum_ref_positions: &HashSet<(String, u32, u32)>,
        unresolved_enum_refs: &HashSet<(String, u32, u32)>,
    ) -> SymbolIndex {
        use llg::ffi::vpi;

        let mut decls: Vec<SymEntry> = Vec::new();
        let mut refs: Vec<SymEntry> = Vec::new();
        let mut claimed: HashSet<(String, u32, u32)> = HashSet::new();
        let mut instance_def: HashMap<usize, String> = HashMap::new();
        let mut port_label_candidates: Vec<PortLabelCandidate> = Vec::new();
        // 0-based positions of scanned parameter-override labels: the exact
        // parse-tree evidence backing `TOKEN_PARAM_CONN_LABEL` tokens (and
        // recorded independently of token classification).
        let param_pair_positions: HashSet<(String, u32, u32)> = pairs
            .iter()
            .filter(|p| p.kind == ConnKind::Param)
            .map(|p| {
                (
                    p.file.clone(),
                    p.label.0.saturating_sub(1),
                    p.label.1.saturating_sub(1),
                )
            })
            .collect();

        // ── Name sets from the model ─────────────────────────────────────────
        let mut signal_names: HashSet<String> = HashSet::new();
        let mut port_names: HashSet<String> = HashSet::new();
        let mut param_names: HashSet<String> = HashSet::new();
        let mut module_names: HashSet<String> = HashSet::new();
        for m in &model.modules {
            module_names.insert(clean_name(&m.name).to_owned());
        }
        for inst in all_instances(&model.top_instances) {
            for p in &inst.ports {
                port_names.insert(p.name.clone());
                signal_names.insert(p.name.clone());
            }
            for s in &inst.signals {
                signal_names.insert(s.name.clone());
            }
            for pa in &inst.params {
                param_names.insert(pa.name.clone());
                signal_names.insert(pa.name.clone());
            }
            for gs in &inst.gen_scopes {
                for pa in &gs.params {
                    param_names.insert(pa.name.clone());
                    signal_names.insert(pa.name.clone());
                }
            }
        }

        let tokens_by_file: HashMap<&str, &FileTokens> =
            tokens.iter().map(|ft| (ft.path.as_str(), ft)).collect();

        // The syntax-fallback declaration set is a HashSet, so walking it
        // directly cannot reuse the token order.  Build the same-position
        // lookup once over the owned token stream; this replaces one
        // `nodes.iter().find` scan per declaration while preserving the old
        // first-node-at-position choice.
        let parse_nodes_by_position = parse_decls.map(|_| {
            let mut nodes_by_position: HashMap<(&str, u32, u32), &llg::ffi::surelog::VObjectInfo> =
                HashMap::with_capacity(tokens.iter().map(|ft| ft.nodes.len()).sum());
            for file_tokens in tokens_by_file.values() {
                for node in &file_tokens.nodes {
                    nodes_by_position
                        .entry((file_tokens.path.as_str(), node.line, node.col))
                        .or_insert(node);
                }
            }
            nodes_by_position
        });

        // ── Parse-fallback declaration seeds ────────────────────────────────
        // Seed the name sets from the collector's recorded declaration
        // positions so `classify_token` accepts them despite the absent
        // instance data.
        if let Some(decl_positions) = parse_decls {
            for (file, line1, col1) in decl_positions {
                let Some(node) = parse_nodes_by_position
                    .as_ref()
                    .and_then(|nodes| nodes.get(&(file.as_str(), *line1, *col1)))
                else {
                    continue;
                };
                let Some(name) = node.name.as_deref() else {
                    continue;
                };
                let nm = name.to_owned();
                match node.vpi_type {
                    vpi::TOKEN_PORT_INPUT | vpi::TOKEN_PORT_OUTPUT | vpi::TOKEN_PORT_INOUT => {
                        port_names.insert(nm.clone());
                        signal_names.insert(nm);
                    }
                    vpi::vpiParameter => {
                        param_names.insert(nm.clone());
                        signal_names.insert(nm);
                    }
                    _ => {
                        signal_names.insert(nm);
                    }
                }
            }
        }

        // ── Module declarations (model; position refined by token) ──────────
        for m in &model.modules {
            let Some(file) = m.file.clone() else { continue };
            let (line1, col1) = tokens_by_file
                .get(file.as_str())
                .and_then(|ft| {
                    ft.nodes.iter().find(|n| {
                        n.vpi_type == vpi::vpiModule
                            && clean_name(n.name.as_deref().unwrap_or("")) == clean_name(&m.name)
                    })
                })
                .map(|n| (n.line, n.col))
                .unwrap_or((m.line, m.col));
            let name = clean_name(&m.name).to_owned();
            let len = lsp_name_len(&name);
            let line = line1.saturating_sub(1);
            let col = col1.saturating_sub(1);
            decls.push(SymEntry {
                name: name.clone(),
                kind: SymKind::Module,
                file: file.clone(),
                line,
                col,
                end_line: line,
                end_col: col.saturating_add(len),
                is_decl: true,
                scope: None,
                detail: Some(format!("module {name}")),
            });
            claimed.insert((file, line1, col1));
        }

        // ── Package declarations ────────────────────────────────────────────
        for p in &model.packages {
            let Some(file) = p.file.clone() else { continue };
            let (line1, col1) = tokens_by_file
                .get(file.as_str())
                .and_then(|ft| {
                    ft.nodes.iter().find(|n| {
                        n.vpi_type == vpi::uhdmpackage
                            && clean_name(n.name.as_deref().unwrap_or("")) == clean_name(&p.name)
                    })
                })
                .map(|n| (n.line, n.col))
                .unwrap_or((p.line, p.col));
            let name = clean_name(&p.name).to_owned();
            let len = lsp_name_len(&name);
            let line = line1.saturating_sub(1);
            let col = col1.saturating_sub(1);
            decls.push(SymEntry {
                name: name.clone(),
                kind: SymKind::Package,
                file: file.clone(),
                line,
                col,
                end_line: line,
                end_col: col.saturating_add(len),
                is_decl: true,
                scope: None,
                detail: Some(format!("package {name}")),
            });
            claimed.insert((file, line1, col1));
        }

        // ── Package item declarations (model) ───────────────────────────────
        // Parameters and enum constants of every package, positioned from the
        // model (enum consts) or from the declaration token in the package
        // file (params — `ParamModel` carries no position).  These win over
        // the token pass via `claimed`, so their hover detail (type/value)
        // comes from the model.
        for p in &model.packages {
            let Some(file) = p.file.clone() else { continue };
            let pkg_name = clean_name(&p.name).to_owned();
            let ft = tokens_by_file.get(file.as_str());
            for param in &p.params {
                // `ParamModel` carries no position; the package-file token
                // (vpiParameter at the declaration site) supplies it.  Params
                // without a token are skipped rather than mis-positioned.
                let Some((line1, col1)) = ft
                    .and_then(|ft| {
                        ft.nodes.iter().find(|n| {
                            n.vpi_type == vpi::vpiParameter
                                && clean_name(n.name.as_deref().unwrap_or("")) == param.name
                        })
                    })
                    .map(|n| (n.line, n.col))
                else {
                    continue;
                };
                let name = clean_name(&param.name).to_owned();
                let len = lsp_name_len(&name);
                let line = line1.saturating_sub(1);
                let col = col1.saturating_sub(1);
                decls.push(SymEntry {
                    name,
                    kind: SymKind::Param,
                    file: file.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl: true,
                    scope: Some(pkg_name.clone()),
                    detail: Some(format_param(param)),
                });
                claimed.insert((file.clone(), line1, col1));
            }
            for ec in &p.enum_consts {
                let name = clean_name(&ec.name).to_owned();
                let len = lsp_name_len(&name);
                let line = ec.line.saturating_sub(1);
                let col = ec.col.saturating_sub(1);
                decls.push(SymEntry {
                    name,
                    kind: SymKind::EnumConst,
                    file: file.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl: true,
                    scope: Some(pkg_name.clone()),
                    detail: Some(format_enum_const(ec)),
                });
                claimed.insert((file.clone(), ec.line, ec.col));
            }
        }

        // ── Class declarations and class members (model) ───────────────────
        // Classes are per-file definitions (not per-instance clones).  The
        // class decl position is refined to the class *name* token (Surelog's
        // own position points at the `class` keyword); methods and fields are
        // indexed with the class name as their scope so `Class::member`
        // resolution and class-scoped completion work.  Surelog's builtin
        // classes (mailbox/process/semaphore) report a virtual `<cwd>/builtin.sv`
        // file that never exists on disk, so they are skipped the same way the
        // builtin package is (no user code to navigate to).
        for c in &model.classes {
            let Some(file) = c.file.clone() else { continue };
            if builtin_file(&file) {
                continue;
            }
            let (line1, col1) = tokens_by_file
                .get(file.as_str())
                .and_then(|ft| {
                    ft.nodes.iter().find(|n| {
                        n.vpi_type == vpi::uhdmclass_defn
                            && n.line == c.line
                            && clean_name(n.name.as_deref().unwrap_or("")) == clean_name(&c.name)
                    })
                })
                .map(|n| (n.line, n.col))
                .unwrap_or((c.line, c.col));
            let name = clean_name(&c.name).to_owned();
            let len = lsp_name_len(&name);
            let line = line1.saturating_sub(1);
            let col = col1.saturating_sub(1);
            decls.push(SymEntry {
                name: name.clone(),
                kind: SymKind::Class,
                file: file.clone(),
                line,
                col,
                end_line: line,
                end_col: col.saturating_add(len),
                is_decl: true,
                scope: None,
                detail: Some(format_class(c)),
            });
            claimed.insert((file.clone(), line1, col1));

            let scope = Some(name.clone());
            for m in &c.methods {
                let Some(mfile) = m.file.clone() else {
                    continue;
                };
                let mname = clean_name(&m.name).to_owned();
                let mlen = lsp_name_len(&mname);
                let mline = m.line.saturating_sub(1);
                let mcol = m.col.saturating_sub(1);
                decls.push(SymEntry {
                    name: mname.clone(),
                    kind: if m.is_task {
                        SymKind::Task
                    } else {
                        SymKind::Function
                    },
                    file: mfile.clone(),
                    line: mline,
                    col: mcol,
                    end_line: mline,
                    end_col: mcol.saturating_add(mlen),
                    is_decl: true,
                    scope: scope.clone(),
                    detail: Some(func_signature(m)),
                });
                claimed.insert((mfile.clone(), m.line, m.col));
                // Surelog's own method position points at the `function`/
                // `task` keyword; the parse-tree name token sits on the same
                // line.  Claim it too so the token pass does not emit a
                // second, detail-less method decl at the name.
                if let Some((l1, c1)) = tokens_by_file
                    .get(file.as_str())
                    .and_then(|ft| {
                        ft.nodes.iter().find(|n| {
                            (n.vpi_type == vpi::vpiFunction || n.vpi_type == vpi::vpiTask)
                                && n.line == m.line
                                && clean_name(n.name.as_deref().unwrap_or("")) == mname
                        })
                    })
                    .map(|n| (n.line, n.col))
                {
                    claimed.insert((file.clone(), l1, c1));
                }
            }
            // Class fields reuse `SymKind::Var` (no dedicated Field kind).
            for f in &c.fields {
                let fname = clean_name(&f.name).to_owned();
                let flen = lsp_name_len(&fname);
                let fline = f.line.saturating_sub(1);
                let fcol = f.col.saturating_sub(1);
                decls.push(SymEntry {
                    name: fname.clone(),
                    kind: SymKind::Var,
                    file: file.clone(),
                    line: fline,
                    col: fcol,
                    end_line: fline,
                    end_col: fcol.saturating_add(flen),
                    is_decl: true,
                    scope: scope.clone(),
                    detail: Some(format_class_field(f)),
                });
                claimed.insert((file.clone(), f.line, f.col));
            }
        }

        // ── Instance declarations (model, at the instantiation site) ────────
        // Surelog's `vpiColumnNo` on an instance points at the module *type*
        // name; the position is refined to the instance-name token (when
        // present) so `entry_at` hits the identifier the user actually clicks.
        fn push_instances(
            insts: &[InstanceModel],
            parent_scope: Option<String>,
            tokens_by_file: &HashMap<&str, &FileTokens>,
            decls: &mut Vec<SymEntry>,
            claimed: &mut HashSet<(String, u32, u32)>,
            instance_def: &mut HashMap<usize, String>,
        ) {
            for i in insts {
                let scope = parent_scope.clone();
                if let Some(file) = i.file.clone() {
                    let name = clean_name(&i.name).to_owned();
                    let (line1, col1) = tokens_by_file
                        .get(file.as_str())
                        .and_then(|ft| {
                            ft.nodes.iter().find(|n| {
                                n.line == i.line
                                    && clean_name(n.name.as_deref().unwrap_or("")) == name
                                    && INSTANCE_NAME_TOKEN_TYPES.contains(&n.vpi_type)
                            })
                        })
                        .map(|n| (n.line, n.col))
                        .unwrap_or((i.line, i.col));
                    let len = lsp_name_len(&name);
                    let line = line1.saturating_sub(1);
                    let col = col1.saturating_sub(1);
                    let def = clean_name(&i.def_name).to_owned();
                    let detail = format!("{} {}\n\ndefined at {file}:{line1}", def, i.name);
                    let idx = decls.len();
                    decls.push(SymEntry {
                        name: name.clone(),
                        kind: SymKind::Instance,
                        file: file.clone(),
                        line,
                        col,
                        end_line: line,
                        end_col: col.saturating_add(len),
                        is_decl: true,
                        scope,
                        detail: Some(detail),
                    });
                    instance_def.insert(idx, def);
                    claimed.insert((file, line1, col1));
                }
                push_instances(
                    &i.children,
                    Some(i.full_name.clone()),
                    tokens_by_file,
                    decls,
                    claimed,
                    instance_def,
                );
            }
        }
        push_instances(
            &model.top_instances,
            None,
            &tokens_by_file,
            &mut decls,
            &mut claimed,
            &mut instance_def,
        );

        // ── Function/task declarations (model, per-instance clones) ─────────
        // Every instance carries its elaborated function/task clones with the
        // definition file/position; these win over the token-based entries
        // (which remain a fallback for clones the model did not capture).
        for inst in all_instances(&model.top_instances) {
            for f in &inst.funcs {
                let Some(file) = f.file.clone() else { continue };
                let name = clean_name(&f.name).to_owned();
                let len = lsp_name_len(&name);
                let line = f.line.saturating_sub(1);
                let col = f.col.saturating_sub(1);
                decls.push(SymEntry {
                    name: name.clone(),
                    kind: if f.is_task {
                        SymKind::Task
                    } else {
                        SymKind::Function
                    },
                    file: file.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl: true,
                    scope: Some(f.scope.clone()),
                    detail: Some(func_signature(f)),
                });
                claimed.insert((file, f.line, f.col));
            }
        }

        // Parse-backed enum declarations supplement the model.  In a valid
        // design package enum constants already arrived through UHDM and were
        // inserted above, so this pass runs afterward and keeps the richer
        // model entry at duplicate positions while retaining class-local and
        // syntax-broken declarations absent from UHDM.
        for parsed in parse_enum_decls {
            let key = (
                parsed.file.clone(),
                parsed.line1.saturating_sub(1),
                parsed.col1.saturating_sub(1),
            );
            if decls
                .iter()
                .any(|d| d.file == key.0 && d.line == key.1 && d.col == key.2)
            {
                continue;
            }
            decls.push(SymEntry {
                name: parsed.name.clone(),
                kind: SymKind::EnumConst,
                file: parsed.file.clone(),
                line: key.1,
                col: key.2,
                end_line: key.1,
                end_col: key.2.saturating_add(lsp_name_len(&parsed.name)),
                is_decl: true,
                scope: parsed.scope.clone(),
                detail: Some(format!("enum constant {}", parsed.name)),
            });
        }

        // ── Scope providers (module/package/class/... decls) for ref scopes ──
        let scope_providers: Vec<SymEntry> = decls
            .iter()
            .filter(|d| {
                matches!(
                    d.kind,
                    SymKind::Module
                        | SymKind::Interface
                        | SymKind::Package
                        | SymKind::Class
                        | SymKind::Program
                )
            })
            .cloned()
            .collect();

        fn enclosing_scope(providers: &[SymEntry], file: &str, line1: u32) -> Option<String> {
            let line0 = line1.saturating_sub(1);
            providers
                .iter()
                .filter(|d| d.file == file && d.line <= line0)
                .max_by_key(|d| d.line)
                .map(|d| d.name.clone())
        }

        // ── Token pass: ports/signals/params decls and references ───────────
        for ft in tokens {
            // Type histogram per position drives the decl/ref classification.
            let mut pos_types: HashMap<(u32, u32), Vec<i32>> = HashMap::new();
            for n in &ft.nodes {
                if n.name.is_some() {
                    pos_types
                        .entry((n.line, n.col))
                        .or_default()
                        .push(n.vpi_type);
                }
            }

            for n in &ft.nodes {
                let Some(name) = n.name.as_deref() else {
                    continue;
                };
                if name.is_empty() || claimed.contains(&(ft.path.clone(), n.line, n.col)) {
                    continue;
                }
                let forced_decl =
                    parse_decls.is_some_and(|set| set.contains(&(ft.path.clone(), n.line, n.col)));
                let Some((kind, is_decl)) = classify_token(
                    n.vpi_type,
                    name,
                    &pos_types,
                    n.line,
                    n.col,
                    &signal_names,
                    &port_names,
                    &param_names,
                    &module_names,
                    forced_decl,
                    parse_enum_ref_positions.contains(&(
                        ft.path.clone(),
                        n.line.saturating_sub(1),
                        n.col.saturating_sub(1),
                    )),
                ) else {
                    continue;
                };
                // Named port connections: a `vpiFunction`/`vpiTask` reference
                // with an instance declaration on the same or an earlier line
                // (see `port_label_candidate` for the exact heuristic), or a
                // classifier-labeled `TOKEN_PORT_CONN_LABEL` token — the same
                // structural evidence, needing no name gate.
                if !is_decl
                    && kind == SymKind::Var
                    && (n.vpi_type == vpi::vpiFunction
                        || n.vpi_type == vpi::vpiTask
                        || n.vpi_type == vpi::TOKEN_PORT_CONN_LABEL)
                {
                    if let Some(cand) = port_label_candidate(
                        &decls,
                        &instance_def,
                        &ft.path,
                        n.line.saturating_sub(1),
                        n.col.saturating_sub(1),
                        name,
                    ) {
                        port_label_candidates.push(cand);
                    }
                }
                let len = lsp_name_len(name);
                let line = n.line.saturating_sub(1);
                let col = n.col.saturating_sub(1);
                let scope = enclosing_scope(&scope_providers, &ft.path, n.line);
                let detail = if is_decl {
                    match kind {
                        SymKind::Port | SymKind::Net | SymKind::Var | SymKind::Param => {
                            decl_detail(model, kind, name)
                        }
                        _ => Some(format!("{} {name}", kind_label(kind))),
                    }
                } else {
                    None
                };
                let entry = SymEntry {
                    name: name.to_owned(),
                    kind,
                    file: ft.path.clone(),
                    line,
                    col,
                    end_line: line,
                    end_col: col.saturating_add(len),
                    is_decl,
                    scope,
                    detail,
                };
                if is_decl {
                    decls.push(entry);
                } else {
                    refs.push(entry);
                }
            }
        }

        // ── Deduplicate by position (keep the first / richest entry) ────────
        let mut seen: HashSet<(String, u32, u32)> = HashSet::new();
        decls.retain(|e| seen.insert((e.file.clone(), e.line, e.col)));
        seen.clear();
        refs.retain(|e| seen.insert((e.file.clone(), e.line, e.col)));

        // ── Named port connections → child port declarations ───────────────
        // Resolved after dedup so `decls` indices are final; declarations
        // synthesized for missing port entries are appended here, before the
        // lookup maps are built.
        let ref_positions: HashSet<(String, u32, u32)> = refs
            .iter()
            .map(|r| (r.file.clone(), r.line, r.col))
            .collect();
        let mut port_labels: HashMap<(String, u32, u32), usize> = HashMap::new();
        for cand in &port_label_candidates {
            let Some(((pos_file, pos_line, pos_col), idx)) =
                resolve_port_label(cand, model, &mut decls)
            else {
                continue; // not a genuine named connection (e.g. function call)
            };
            if ref_positions.contains(&(pos_file.clone(), pos_line, pos_col)) {
                port_labels.insert((pos_file, pos_line, pos_col), idx);
            }
        }

        // ── Named parameter overrides → child parameter declarations ───────
        // Resolved straight from the scanned pairs (exact parse-tree
        // evidence; no positional heuristic — in valid SV the override list
        // `child #(.W(4)) u0 (...)` PRECEDES the instance name, so the port
        // label's same-line rule cannot apply).  Every scanned override
        // label position that did NOT resolve is remembered so definition
        // requests there yield no result instead of a wrong same-name jump.
        let mut param_labels: HashMap<(String, u32, u32), usize> = HashMap::new();
        for pair in pairs.iter().filter(|p| p.kind == ConnKind::Param) {
            if let Some(((pos_file, pos_line, pos_col), idx)) =
                resolve_param_override(pair, model, &mut decls)
            {
                if ref_positions.contains(&(pos_file.clone(), pos_line, pos_col)) {
                    param_labels.insert((pos_file, pos_line, pos_col), idx);
                }
            }
        }
        let unresolved_param_labels: HashSet<(String, u32, u32)> = param_pair_positions
            .iter()
            .filter(|pos| !param_labels.contains_key(pos))
            .cloned()
            .collect();
        let unresolved_enum_refs = unresolved_enum_refs.clone();

        // ── Lookup maps ──────────────────────────────────────────────────────
        let mut decls_by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, d) in decls.iter().enumerate() {
            decls_by_name.entry(d.name.clone()).or_default().push(i);
        }
        let mut decls_by_file: HashMap<String, Vec<SymEntry>> = HashMap::new();
        for d in &decls {
            decls_by_file
                .entry(d.file.clone())
                .or_default()
                .push(d.clone());
        }
        let mut refs_by_file: HashMap<String, Vec<SymEntry>> = HashMap::new();
        for r in &refs {
            refs_by_file
                .entry(r.file.clone())
                .or_default()
                .push(r.clone());
        }
        for v in decls_by_file.values_mut() {
            v.sort_by_key(|e| (e.line, e.col));
        }
        for v in refs_by_file.values_mut() {
            v.sort_by_key(|e| (e.line, e.col));
        }

        SymbolIndex {
            decls,
            refs,
            decls_by_file,
            refs_by_file,
            decls_by_name,
            instance_def,
            port_labels,
            param_labels,
            unresolved_param_labels,
            unresolved_enum_refs,
        }
    }

    /// The symbol entry whose range covers the 0-based `(line, col)` position
    /// in `file` (declarations and references merged; the longest name wins so
    /// nested identifiers like `pkg::item` match their full spelling).
    pub fn entry_at(&self, file: &str, line: u32, col: u32) -> Option<&SymEntry> {
        // Parse-backed qualified enum references can coexist with Surelog's
        // folded `pkg::member` token.  An exact token start is the most
        // precise cursor anchor, so prefer it before the broader spelling's
        // containing range.
        if let Some(exact) = self
            .decls_in_file(file)
            .iter()
            .chain(self.refs_in_file(file).iter())
            .find(|entry| entry.line == line && entry.col == col)
        {
            return Some(exact);
        }
        let mut best: Option<&SymEntry> = None;
        let mut best_len: usize = 0;
        for e in self
            .decls_in_file(file)
            .iter()
            .chain(self.refs_in_file(file).iter())
        {
            if e.line != line {
                continue;
            }
            let len = lsp_name_len(&e.name) as usize;
            if col >= e.col && col < e.col.saturating_add(len as u32) && len > best_len {
                best = Some(e);
                best_len = len;
            }
        }
        best
    }

    /// Declarations in `file`, sorted by (line, col).
    pub fn decls_in_file(&self, file: &str) -> &[SymEntry] {
        self.decls_by_file.get(file).map_or(&[], |v| v.as_slice())
    }

    /// References in `file`, sorted by (line, col).
    fn refs_in_file(&self, file: &str) -> &[SymEntry] {
        self.refs_by_file.get(file).map_or(&[], |v| v.as_slice())
    }

    /// Resolve `e` to the declaration(s) it refers to.
    ///
    /// Rules (v1, documented):
    /// 1. A named connection label resolves to the CHILD module's
    ///    declaration (precomputed in [`SymbolIndex::from_parts`], cross-file
    ///    included): a port label (`.clk` in `m u0(.clk(c))`) to the port, a
    ///    parameter override label (`W` in `m u0 #(.W(4)) (...)`) to the
    ///    parameter.
    /// 2. An `Instance` declaration resolves to the module definition named by
    ///    its `def_name` (stored during the build).
    /// 3. Package-qualified names (`pkg::item` in either spelling — the token
    ///    name may be the full `pkg::item` or just `item` when the reference
    ///    sits inside package scope) resolve to the named package's item
    ///    declarations (parameters and enum constants by name, cross-file);
    ///    `pkg` alone resolves to the package declaration.
    ///    Parse-backed enum bindings take precedence at the exact member
    ///    coordinate, including class scopes and imported bare members.
    /// 4. A reference resolves to: the same-named declaration in the same
    ///    scope, else the same-named declaration in the same file nearest by
    ///    line, else any same-named declaration in the workspace.  Module-type
    ///    references apply that order only to module declarations, so an
    ///    instance with the same name cannot capture the type reference.
    ///    Scanned parameter-override labels that failed rule 1 are excluded:
    ///    their namespace is the INSTANTIATED module, so a same-name
    ///    declaration of the instantiating scope or an arbitrary workspace
    ///    match is wrong by construction — they resolve to nothing instead.
    /// 5. Any other declaration resolves to itself.
    pub fn resolve<'a>(&'a self, e: &'a SymEntry) -> Vec<&'a SymEntry> {
        // Named connection labels resolve to the child module's port /
        // parameter declaration, computed at index-build time.
        if !e.is_decl {
            let key = (e.file.clone(), e.line, e.col);
            if let Some(&idx) = self.port_labels.get(&key) {
                if let Some(d) = self.decls.get(idx) {
                    return vec![d];
                }
            }
            if let Some(&idx) = self.param_labels.get(&key) {
                if let Some(d) = self.decls.get(idx) {
                    return vec![d];
                }
            }
            if self.unresolved_param_labels.contains(&key) {
                return Vec::new();
            }
            if self.unresolved_enum_refs.contains(&key) {
                return Vec::new();
            }
        }
        if let Some((pkg, item)) = e.name.split_once("::") {
            let mut out: Vec<&SymEntry> = Vec::new();
            // Item declarations scoped to the named package or class.
            // `clean_name` handles both the bare spelling and a `work@my_pkg`
            // prefix.  Class members carry the (library-stripped) class name
            // as their scope, so `Counter::get` resolves the same way
            // `my_pkg::P` does.
            if let Some(indices) = self.decls_by_name.get(item) {
                out.extend(indices.iter().filter_map(|&i| {
                    let d = &self.decls[i];
                    let in_scope = d
                        .scope
                        .as_deref()
                        .map(|s| clean_name(s) == clean_name(pkg))
                        .unwrap_or(false);
                    (in_scope
                        && matches!(
                            d.kind,
                            SymKind::Param
                                | SymKind::EnumConst
                                | SymKind::Typedef
                                | SymKind::Function
                                | SymKind::Task
                                | SymKind::Var
                        ))
                    .then_some(d)
                }));
            }
            if !out.is_empty() {
                return out;
            }
            // Fallback: the package/class declaration plus any same-named
            // declarations workspace-wide (unchanged v1 behavior).
            if let Some(indices) = self.decls_by_name.get(pkg) {
                out.extend(indices.iter().filter_map(|&i| {
                    matches!(self.decls[i].kind, SymKind::Package | SymKind::Class)
                        .then_some(&self.decls[i])
                }));
            }
            if let Some(indices) = self.decls_by_name.get(item) {
                out.extend(indices.iter().map(|&i| &self.decls[i]));
            }
            return out;
        }
        if e.is_decl {
            if e.kind == SymKind::Instance {
                if let Some(def) = self.instance_def_of(e) {
                    let mut out: Vec<&SymEntry> = Vec::new();
                    if let Some(indices) = self.decls_by_name.get(&def) {
                        out.extend(indices.iter().filter_map(|&i| {
                            let d = &self.decls[i];
                            matches!(
                                d.kind,
                                SymKind::Module
                                    | SymKind::Interface
                                    | SymKind::Package
                                    | SymKind::Class
                                    | SymKind::Program
                            )
                            .then_some(d)
                        }));
                    }
                    if !out.is_empty() {
                        return out;
                    }
                }
            }
            return vec![e];
        }
        // Reference: in-scope declaration first, then same-file, then
        // workspace-wide.  Module-type references occupy the module namespace;
        // ordinary identifier references retain the historical name-only
        // behavior.
        let module_type_reference = e.kind == SymKind::Module;
        if let Some(indices) = self.decls_by_name.get(&e.name) {
            let in_scope: Vec<&SymEntry> = indices
                .iter()
                .filter(|&&i| {
                    self.decls[i].scope == e.scope
                        && (!module_type_reference || self.decls[i].kind == SymKind::Module)
                })
                .map(|&i| &self.decls[i])
                .collect();
            if !in_scope.is_empty() {
                return in_scope;
            }
            let mut same_file: Vec<&SymEntry> = indices
                .iter()
                .filter(|&&i| {
                    self.decls[i].file == e.file
                        && (!module_type_reference || self.decls[i].kind == SymKind::Module)
                })
                .map(|&i| &self.decls[i])
                .collect();
            same_file.sort_by_key(|d| d.line.abs_diff(e.line));
            if !same_file.is_empty() {
                return same_file;
            }
            let all: Vec<&SymEntry> = indices
                .iter()
                .filter(|&&i| !module_type_reference || self.decls[i].kind == SymKind::Module)
                .map(|&i| &self.decls[i])
                .collect();
            if !all.is_empty() {
                return all;
            }
        }
        Vec::new()
    }

    /// The declaration at the head of `e` plus every reference resolving to
    /// the same declaration, deduplicated by position.
    ///
    /// When resolution fails (e.g. an unknown name), all same-named
    /// declarations are used as the resolution head.
    pub fn all_references(&self, e: &SymEntry) -> Vec<SymEntry> {
        let mut resolved: Vec<&SymEntry> = self.resolve(e);
        let unresolved_enum = !e.is_decl
            && self
                .unresolved_enum_refs
                .contains(&(e.file.clone(), e.line, e.col));
        if resolved.is_empty() && !unresolved_enum {
            if let Some(indices) = self.decls_by_name.get(&e.name) {
                resolved = indices.iter().map(|&i| &self.decls[i]).collect();
            }
        }
        let resolved_set: HashSet<(String, u32, u32)> = resolved
            .iter()
            .map(|d| (d.file.clone(), d.line, d.col))
            .collect();
        let mut out: Vec<SymEntry> = Vec::new();
        let mut seen: HashSet<(String, u32, u32)> = HashSet::new();
        for d in &resolved {
            if seen.insert((d.file.clone(), d.line, d.col)) {
                out.push((*d).clone());
            }
        }
        for r in &self.refs {
            let hits = self
                .resolve(r)
                .iter()
                .any(|d| resolved_set.contains(&(d.file.clone(), d.line, d.col)));
            if hits && seen.insert((r.file.clone(), r.line, r.col)) {
                out.push(r.clone());
            }
        }
        out
    }

    /// Whether `(file, line, col)` is a scanned named PARAMETER override
    /// label position that did not resolve to a child-module declaration.
    ///
    /// Definition serving uses this to return NO result for such labels: their
    /// namespace is the instantiated module, so every name-based fallback
    /// (same scope / same file / workspace) would be wrong by construction.
    pub fn is_unresolved_param_label(&self, file: &str, line: u32, col: u32) -> bool {
        self.unresolved_param_labels
            .contains(&(file.to_owned(), line, col))
    }

    /// Whether a parse-backed enum reference was intentionally left
    /// unresolved because its visible declarations were ambiguous.
    pub fn is_unresolved_enum_ref(&self, file: &str, line: u32, col: u32) -> bool {
        self.unresolved_enum_refs
            .contains(&(file.to_owned(), line, col))
    }

    /// The decl index of `e`, used to look up stored per-decl data.
    fn instance_def_of(&self, e: &SymEntry) -> Option<String> {
        let idx = self.decls.iter().position(|d| d == e)?;
        self.instance_def.get(&idx).cloned()
    }
}

/// A detected named port connection (`.clk` in `m u0(.clk(c))`) awaiting
/// resolution to the child module's port declaration.
pub(super) struct PortLabelCandidate {
    /// 0-based position of the label identifier (the `.clk` text).
    file: String,
    line: u32,
    col: u32,
    /// The label text == the child port name.
    name: String,
    /// The associated instance name (e.g. `u0`).
    inst_name: String,
    /// The instance's module definition name (e.g. `m`).
    def_name: String,
    /// 0-based line of the associated instance declaration: the same line for
    /// same-line labels, an earlier line for continuation-line labels.
    inst_line: u32,
}

/// Detect a named connection label (port or parameter override) at an
/// indexed reference site.
///
/// The index has no source text, so the check is positional (verified against
/// the hand-built fixtures in the test module):
///
/// 1. the token at the position was classified as a *reference* (not a decl)
///    of kind `Var` by the `vpiFunction`/`vpiTask` heuristic — the existing
///    signal-name test already excludes real function/task declarations;
/// 2. **same-line rule** (v1, unchanged): the same line holds an `Instance`
///    declaration at an earlier column (the instance name); the label column
///    must be at least `instance name length + 2` past it, i.e. room for the
///    `(` and `.` that separate `<name>` from the label — this also rejects
///    hierarchical references like `u0.clk` (gap 1) and
///    first-connection-less positions; the rightmost such instance wins, so
///    several instantiations on one line attribute each label to its own
///    instance;
/// 3. **continuation-line rule** (multi-line instantiations like
///    `m u0(\n  .clk(c),\n  .o(o)\n);`): when no same-line instance exists,
///    the label is associated with the nearest preceding `Instance`
///    declaration in the same file (highest line, then rightmost column)
///    provided that:
///    - no other `Instance`, `Module`, or `Package` declaration lies strictly
///      between the instance line and the label line (the label cannot belong
///      to a different scope or to a later instantiation),
///    - the label line is within [`PORT_LABEL_MAX_SPAN`] lines of the
///      instance line,
///    - the label column is > 0, i.e. the connection is indented (a
///      continuation-line connection is never flush against the left margin).
///
/// [`resolve_port_label`] then verifies the name is an actual port of that
/// instance, which rejects function calls and other false positives.
pub(super) fn port_label_candidate(
    decls: &[SymEntry],
    instance_def: &HashMap<usize, String>,
    file: &str,
    line: u32,
    col: u32,
    name: &str,
) -> Option<PortLabelCandidate> {
    // ── Same-line rule ─────────────────────────────────────────────────────
    if let Some((idx, inst)) = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            d.kind == SymKind::Instance && d.file == file && d.line == line && d.col < col
        })
        .max_by_key(|(_, d)| d.col)
    {
        let min_label_col = inst.col.saturating_add(lsp_name_len(&inst.name) + 2);
        if col >= min_label_col {
            let def_name = instance_def.get(&idx)?.clone();
            return Some(PortLabelCandidate {
                file: file.to_owned(),
                line,
                col,
                name: name.to_owned(),
                inst_name: inst.name.clone(),
                def_name,
                inst_line: line,
            });
        }
    }

    // ── Continuation-line rule ─────────────────────────────────────────────
    if col == 0 || line == 0 {
        return None;
    }
    let (idx, inst) = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind == SymKind::Instance && d.file == file && d.line < line)
        .max_by_key(|(_, d)| (d.line, d.col))?;
    if line - inst.line > PORT_LABEL_MAX_SPAN {
        return None;
    }
    let scope_blocked = decls.iter().any(|d| {
        d.file == file
            && d.line > inst.line
            && d.line < line
            && matches!(
                d.kind,
                SymKind::Instance | SymKind::Module | SymKind::Package
            )
    });
    if scope_blocked {
        return None;
    }
    let def_name = instance_def.get(&idx)?.clone();
    Some(PortLabelCandidate {
        file: file.to_owned(),
        line,
        col,
        name: name.to_owned(),
        inst_name: inst.name.clone(),
        def_name,
        inst_line: inst.line,
    })
}

/// Resolve a detected named port connection to the child module's port
/// declaration.
///
/// Returns `(label position, decl index)` when the label is a genuine named
/// connection; `None` when the name is not a port of the instance (a function
/// call or a false positive) or the instance's module definition cannot be
/// found — the caller then falls back to ordinary name-based resolution.
///
/// The port declaration is looked up by (name, def file, kind `Port`),
/// preferring the entry whose scope is the instance's module definition.  When
/// the index has no such declaration (hand-built models), one is synthesized
/// anchored at the module header, offset by the port index so multiple missing
/// ports of the same module keep unique positions.
pub(super) fn resolve_port_label(
    cand: &PortLabelCandidate,
    model: &DesignModel,
    decls: &mut Vec<SymEntry>,
) -> Option<((String, u32, u32), usize)> {
    let inst = find_instance(
        &model.top_instances,
        &cand.inst_name,
        &cand.file,
        cand.inst_line,
    )?;
    let port = inst.ports.iter().find(|p| p.name == cand.name)?;
    let module = model
        .modules
        .iter()
        .find(|m| clean_name(&m.name) == clean_name(&cand.def_name))?;
    let def_file = module.file.as_ref()?;
    let scope = clean_name(&cand.def_name).to_owned();
    let key = (cand.file.clone(), cand.line, cand.col);

    let in_def_file: Vec<(usize, &SymEntry)> = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            d.kind == SymKind::Port && d.name == cand.name && d.file == def_file.as_str()
        })
        .collect();
    // Prefer the port declaration of the instance's module definition.
    if let Some(&(idx, _)) = in_def_file
        .iter()
        .find(|(_, d)| d.scope.as_deref() == Some(scope.as_str()))
    {
        return Some((key, idx));
    }
    // A single unscoped port declaration of the name in the def file.
    if in_def_file.len() == 1 && in_def_file[0].1.scope.is_none() {
        return Some((key, in_def_file[0].0));
    }
    // No indexed declaration: synthesize one (reusing an existing entry at
    // the anchor position, e.g. from a previous label of the same port).
    let module_name_len = lsp_name_len(clean_name(&module.name));
    let port_idx = inst
        .ports
        .iter()
        .position(|p| p.name == cand.name)
        .unwrap_or(0) as u32;
    let line0 = module.line.saturating_sub(1);
    let col0 = module.col.saturating_sub(1) + module_name_len + port_idx;
    if let Some((idx, _)) = decls
        .iter()
        .enumerate()
        .find(|(_, d)| d.file == def_file.as_str() && d.line == line0 && d.col == col0)
    {
        return Some((key, idx));
    }
    let len = lsp_name_len(&cand.name);
    let idx = decls.len();
    decls.push(SymEntry {
        name: cand.name.clone(),
        kind: SymKind::Port,
        file: def_file.clone(),
        line: line0,
        col: col0,
        end_line: line0,
        end_col: col0 + len,
        is_decl: true,
        scope: Some(scope),
        detail: Some(format_port(port)),
    });
    Some((key, idx))
}

/// Column stride that keeps synthesized PARAMETER anchors disjoint from the
/// port anchors [`resolve_port_label`] places at the same module header.
///
/// Synthesized declarations are virtual positions (the index had no real
/// declaration for the name); without the stride a child module's i-th
/// missing parameter and i-th missing port would synthesize onto the SAME
/// header position, and whichever resolved second would silently reuse the
/// first one's entry — pointing parameter labels at port-kind declarations.
pub(super) const PARAM_SYNTH_COL_STRIDE: u32 = 64;

/// Resolve a scanned named PARAMETER override to the child module's
/// parameter declaration.
///
/// Unlike the port path (which guesses the owning instance positionally),
/// the pair itself carries the instantiation's module TYPE (`inst_type`),
/// which IS the definition namespace:
///
/// 1. any elaborated clone of that type supplies the parameter list —
///    preferring a clone in the instantiating file; the pair's label name
///    must be one of its parameters;
/// 2. the declaration is looked up by `(name, def file, kind Param)`,
///    preferring the entry whose scope is the module definition; a single
///    unscoped match in the def file is accepted;
/// 3. otherwise a declaration is synthesized anchored at the module header
///    (offset by [`PARAM_SYNTH_COL_STRIDE`] plus the parameter index so
///    anchors never collide with synthesized ports).
///
/// Returns `None` when the override cannot be tied to a parameter of the
/// instantiated type (unknown type, unknown parameter, or a definition
/// without a file) — the caller records the position as unresolved.
pub(super) fn resolve_param_override(
    pair: &NamedPortConn,
    model: &DesignModel,
    decls: &mut Vec<SymEntry>,
) -> Option<((String, u32, u32), usize)> {
    let def_name = clean_name(pair.inst_type.as_deref()?).to_owned();
    let key = (
        pair.file.clone(),
        pair.label.0.saturating_sub(1),
        pair.label.1.saturating_sub(1),
    );
    let name = pair.label_name.as_str();
    // Any elaborated clone of the instantiated type carries the same
    // parameter list (declaration order included).
    let inst = all_instances(&model.top_instances)
        .into_iter()
        .filter(|i| clean_name(&i.def_name) == def_name)
        .min_by_key(|i| i.file.as_deref() != Some(pair.file.as_str()))?;
    let param = inst.params.iter().find(|p| p.name == name)?;
    let module = model
        .modules
        .iter()
        .find(|m| clean_name(&m.name) == def_name)?;
    let def_file = module.file.as_ref()?;
    let scope = def_name;
    let in_def_file: Vec<(usize, &SymEntry)> = decls
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind == SymKind::Param && d.name == name && d.file == def_file.as_str())
        .collect();
    // Prefer the parameter declaration of the instance's module definition.
    if let Some(&(idx, _)) = in_def_file
        .iter()
        .find(|(_, d)| d.scope.as_deref() == Some(scope.as_str()))
    {
        return Some((key, idx));
    }
    // A single unscoped parameter declaration of the name in the def file.
    if in_def_file.len() == 1 && in_def_file[0].1.scope.is_none() {
        return Some((key, in_def_file[0].0));
    }
    // No indexed declaration: synthesize one (reusing an existing entry at
    // the anchor position, e.g. from a previous label of the same parameter).
    let module_name_len = lsp_name_len(&scope);
    let param_idx = inst.params.iter().position(|p| p.name == name).unwrap_or(0) as u32;
    let line0 = module.line.saturating_sub(1);
    let col0 = module.col.saturating_sub(1) + module_name_len + PARAM_SYNTH_COL_STRIDE + param_idx;
    if let Some((idx, _)) = decls
        .iter()
        .enumerate()
        .find(|(_, d)| d.file == def_file.as_str() && d.line == line0 && d.col == col0)
    {
        return Some((key, idx));
    }
    let len = lsp_name_len(name);
    let idx = decls.len();
    decls.push(SymEntry {
        name: name.to_owned(),
        kind: SymKind::Param,
        file: def_file.clone(),
        line: line0,
        col: col0,
        end_line: line0,
        end_col: col0 + len,
        is_decl: true,
        scope: Some(scope),
        detail: Some(format_param(param)),
    });
    Some((key, idx))
}

/// The instance of `name` in `file`, walking `top_instances` recursively.
///
/// When several instances share the name (different scopes), the one whose
/// instantiation line matches `line` wins.  For continuation-line labels the
/// caller passes the associated instance declaration's line (see
/// [`PortLabelCandidate::inst_line`]), so the exact-line preference still
/// applies; without a line match the first same-name/same-file instance is
/// returned as a fallback.
pub(super) fn find_instance<'m>(
    insts: &'m [InstanceModel],
    name: &str,
    file: &str,
    line: u32,
) -> Option<&'m InstanceModel> {
    let mut matches: Vec<&InstanceModel> = all_instances(insts)
        .into_iter()
        .filter(|i| clean_name(&i.name) == name && i.file.as_deref() == Some(file))
        .collect();
    if let Some(pos) = matches
        .iter()
        .position(|i| i.line.saturating_sub(1) == line)
    {
        return Some(matches.remove(pos));
    }
    matches.into_iter().next()
}

/// Classify a token node as (kind, is_decl); `None` when it is not an indexed
/// symbol site.  See [`SymbolIndex::from_parts`] for the empirical basis.
#[allow(clippy::too_many_arguments)]
pub(super) fn classify_token(
    t: i32,
    name: &str,
    pos_types: &HashMap<(u32, u32), Vec<i32>>,
    line: u32,
    col: u32,
    signal_names: &HashSet<String>,
    port_names: &HashSet<String>,
    param_names: &HashSet<String>,
    module_names: &HashSet<String>,
    forced_decl: bool,
    forced_enum_ref: bool,
) -> Option<(SymKind, bool)> {
    use llg::ffi::vpi;

    // Pure reference types (expression operands walked by the VPI walker).
    if REF_TOKEN_TYPES.contains(&t) {
        return Some((SymKind::Var, false));
    }
    // Named-connection labels are reference sites by construction: the
    // classifier emits the dedicated synthetic types ONLY under
    // `paNamed_port_connection` / `paNamed_parameter_assignment`, and their
    // resolution to the child module's declaration happens through
    // `port_labels`/`param_labels`.  Classifying them as references
    // unconditionally keeps an unresolvable label indexed (cursor
    // normalization, dump visibility) without ever surfacing it as a phantom
    // function/task/parameter DECLARATION.
    if t == vpi::TOKEN_PORT_CONN_LABEL || t == vpi::TOKEN_PARAM_CONN_LABEL {
        return Some((SymKind::Var, false));
    }
    // Port declarations: the direction-specific synthetic types appear only at
    // declaration sites.
    if matches!(
        t,
        vpi::TOKEN_PORT_INPUT | vpi::TOKEN_PORT_OUTPUT | vpi::TOKEN_PORT_INOUT
    ) {
        return Some((SymKind::Port, true));
    }
    // Signal/parameter-like objects.  In the parse-fallback path
    // (`forced_decl`) the position was recorded as a declaration by the core
    // collector, bypassing the instance-derived name gate; single-view parse
    // tokens would otherwise never satisfy the multi-view heuristic below.
    if SIGNAL_DECL_TYPES.contains(&t) {
        if !forced_decl && !signal_names.contains(name) {
            return None;
        }
        let types_at_pos: Vec<i32> = pos_types.get(&(line, col)).cloned().unwrap_or_default();
        let has_ref = types_at_pos.iter().any(|x| REF_TOKEN_TYPES.contains(x));
        let is_decl = forced_decl || (!has_ref && types_at_pos.len() >= 2);
        let kind = if port_names.contains(name) {
            SymKind::Port
        } else if param_names.contains(name) {
            SymKind::Param
        } else if is_net_type(t) {
            SymKind::Net
        } else {
            SymKind::Var
        };
        return Some((kind, is_decl));
    }
    // Module instantiation sites: the type name is a reference to the module
    // definition; class declarations are declarations.
    if t == vpi::uhdmclass_defn {
        if module_names.contains(name) {
            return Some((SymKind::Module, false));
        }
        return Some((SymKind::Class, true));
    }
    // Functions/tasks: named port connections carry the port name (a
    // reference); declarations of functions/tasks are declarations.
    if t == vpi::vpiFunction || t == vpi::vpiTask {
        if signal_names.contains(name) {
            return Some((SymKind::Var, false));
        }
        return Some((
            if t == vpi::vpiFunction {
                SymKind::Function
            } else {
                SymKind::Task
            },
            true,
        ));
    }
    match t {
        vpi::uhdmenum_const => {
            return Some((SymKind::EnumConst, !forced_enum_ref));
        }
        vpi::TOKEN_TYPEDEF_NAME => return Some((SymKind::Typedef, true)),
        vpi::uhdminterface_inst => return Some((SymKind::Interface, true)),
        vpi::vpiProgram | vpi::uhdmprogram => return Some((SymKind::Program, true)),
        _ => {}
    }
    None
}

/// Whether a VPI object type denotes a net (vs. a variable).
pub(super) fn is_net_type(t: i32) -> bool {
    use llg::ffi::vpi;
    matches!(
        t,
        vpi::vpiNet
            | vpi::vpiNetBit
            | vpi::vpiReg
            | vpi::vpiRegBit
            | vpi::vpiLogicVar
            | vpi::uhdmnet
            | vpi::uhdmlogic_net
    )
}

/// Short human label for a symbol kind, used in hover details.
pub(super) fn kind_label(kind: SymKind) -> &'static str {
    match kind {
        SymKind::Module => "module",
        SymKind::Interface => "interface",
        SymKind::Package => "package",
        SymKind::Instance => "instance",
        SymKind::Port => "port",
        SymKind::Net => "net",
        SymKind::Var => "var",
        SymKind::Param => "parameter",
        SymKind::GenScope => "generate scope",
        SymKind::EnumConst => "enum constant",
        SymKind::Typedef => "typedef",
        SymKind::Function => "function",
        SymKind::Task => "task",
        SymKind::Class => "class",
        SymKind::Program => "program",
    }
}

/// Hover text for a port/net/var/param declaration looked up in the model.
pub(super) fn decl_detail(model: &DesignModel, kind: SymKind, name: &str) -> Option<String> {
    match kind {
        SymKind::Port => all_instances(&model.top_instances)
            .into_iter()
            .find_map(|i| i.ports.iter().find(|p| p.name == name).map(format_port)),
        SymKind::Net | SymKind::Var => all_instances(&model.top_instances)
            .into_iter()
            .find_map(|i| i.signals.iter().find(|s| s.name == name).map(format_signal)),
        SymKind::Param => param_display_model(&all_instances(&model.top_instances), name)
            .map(|p| format_param(&p)),
        _ => None,
    }
}

/// The display model for a same-named parameter across all instances.
///
/// The candidate flavor follows the historical precedence — direct non-local
/// parameters first (a same-named `localparam` of the instantiating scope
/// would otherwise shadow the child module's `parameter` in the hover text;
/// connection labels resolve INTO the child, so its parameter is the relevant
/// declaration), then any direct parameter, then generate-scope parameters —
/// but the displayed VALUE must be unanimous across every candidate of the
/// chosen flavor: under divergent per-instance overrides an arbitrary
/// instance's number would be a guess, so the value is cleared instead
/// ([`format_param`] then renders without a value).
pub(super) fn param_display_model(insts: &[&InstanceModel], name: &str) -> Option<ParamModel> {
    fn direct<'m>(insts: &[&'m InstanceModel], name: &str, local: bool) -> Vec<&'m ParamModel> {
        insts
            .iter()
            .filter_map(|i| i.params.iter().find(|p| p.name == name && p.local == local))
            .collect()
    }
    let mut group = direct(insts, name, false);
    if group.is_empty() {
        group = direct(insts, name, true);
    }
    if group.is_empty() {
        group = insts
            .iter()
            .filter_map(|i| {
                i.gen_scopes
                    .iter()
                    .find_map(|gs| gs.params.iter().find(|p| p.name == name))
            })
            .collect();
    }
    let first = group.first()?;
    let mut distinct: Vec<&Val> = Vec::new();
    for p in &group {
        if let Some(v) = &p.value {
            if !distinct.contains(&v) {
                distinct.push(v);
            }
        }
    }
    let mut display = (*first).clone();
    if distinct.len() != 1 {
        display.value = None;
    }
    Some(display)
}

// ── Diagnostics ───────────────────────────────────────────────────────────────
