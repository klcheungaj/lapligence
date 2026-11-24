//! Owned declaration and reference index: symbols, completion and the detail
//! text of declarations. What a name is bound to is [`tokens::Bindings`].

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
/// and the token lists; never touches Slang afterwards.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// All declaration sites, in build order.
    pub decls: Vec<SymEntry>,
    /// All reference sites, in build order.
    pub refs: Vec<SymEntry>,
    /// Declarations per file, sorted by (line, col).
    decls_by_file: HashMap<String, Vec<SymEntry>>,
    /// Decl index → clean module definition name (for `Instance` decls).
    pub(super) instance_def: HashMap<usize, String>,
}

/// Token kinds that denote a module/interface instance name at its site.
pub(super) const INSTANCE_NAME_TOKEN_TYPES: &[i32] = &[
    tokens::TOKEN_SLANG_IDENTIFIER,
    tokens::TOKEN_SLANG_BOUND_IDENTIFIER,
];

impl SymbolIndex {
    /// Merge indexes produced by independent analysis passes.
    ///
    /// Input order is preserved, and duplicate occurrences at the same
    /// position are kept from the first index.  The lookup maps are rebuilt
    /// from the merged vectors so declaration indices remain deterministic.
    /// `instance_def`, whose keys are declaration indices, is remapped as
    /// entries are appended.
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
    }

    pub(super) fn rebuild_lookup_maps(&mut self) {
        self.decls_by_file.clear();

        for decl in &self.decls {
            self.decls_by_file
                .entry(decl.file.clone())
                .or_default()
                .push(decl.clone());
        }
        for entries in self.decls_by_file.values_mut() {
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
    /// * lexical tokens carrying Slang's reference role,
    /// * module type names at instantiation sites,
    /// * named port and parameter labels.
    ///
    /// Slang supplies declaration/reference roles directly. `parse_decls`
    /// retains declarations from a recovery snapshot when elaboration fails.
    pub(super) fn from_parts(
        model: &DesignModel,
        tokens: &[FileTokens],
        parse_decls: Option<&ParseDeclPositions>,
    ) -> SymbolIndex {
        let mut decls: Vec<SymEntry> = Vec::new();
        let mut refs: Vec<SymEntry> = Vec::new();
        let mut claimed: HashSet<(String, u32, u32)> = HashSet::new();
        let mut instance_def: HashMap<usize, String> = HashMap::new();

        let tokens_by_file: HashMap<&str, &FileTokens> =
            tokens.iter().map(|ft| (ft.path.as_str(), ft)).collect();

        // ── Module declarations (model; position refined by token) ──────────
        for m in &model.modules {
            let Some(file) = m.file.clone() else { continue };
            let (line1, col1) = tokens_by_file
                .get(file.as_str())
                .and_then(|ft| {
                    ft.nodes.iter().find(|n| {
                        tokens::token_base_kind(n.kind).0 == tokens::TOKEN_SLANG_MODULE
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
                        tokens::token_base_kind(n.kind).0 == tokens::TOKEN_SLANG_PACKAGE
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
                // at the declaration site supplies it. Params
                // without a token are skipped rather than mis-positioned.
                let Some((line1, col1)) = ft
                    .and_then(|ft| {
                        ft.nodes.iter().find(|n| {
                            matches!(
                                tokens::token_base_kind(n.kind).0,
                                tokens::TOKEN_SLANG_PARAMETER | tokens::TOKEN_SLANG_TYPE_PARAMETER
                            ) && clean_name(n.name.as_deref().unwrap_or("")) == param.name
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
        // class decl position is refined to the class *name* token (Slang's
        // own position points at the `class` keyword); methods and fields are
        // indexed with the class name as their scope so `Class::member`
        // resolution and class-scoped completion work.  Slang's builtin
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
                        tokens::token_base_kind(n.kind).0 == tokens::TOKEN_SLANG_CLASS
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
                // Slang's own method position points at the `function`/
                // `task` keyword; the parse-tree name token sits on the same
                // line.  Claim it too so the token pass does not emit a
                // second, detail-less method decl at the name.
                if let Some((l1, c1)) = tokens_by_file
                    .get(file.as_str())
                    .and_then(|ft| {
                        ft.nodes.iter().find(|n| {
                            matches!(
                                tokens::token_base_kind(n.kind).0,
                                tokens::TOKEN_SLANG_FUNCTION
                                    | tokens::TOKEN_SLANG_TASK
                                    | tokens::TOKEN_SLANG_STATIC_FUNCTION
                            ) && n.line == m.line
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
        // The model's instance position can point at the module type name;
        // refine it to the instance-name token (when
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
                                    && INSTANCE_NAME_TOKEN_TYPES.contains(&n.kind)
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
            let genvar_reference_positions: HashSet<(u32, u32)> = ft
                .nodes
                .iter()
                .filter(|node| node.kind == llg::core::tokens::TOKEN_GENVAR_REF)
                .map(|node| (node.line, node.col))
                .collect();
            for n in &ft.nodes {
                if genvar_reference_positions.contains(&(n.line, n.col))
                    && n.kind != llg::core::tokens::TOKEN_GENVAR_REF
                {
                    continue;
                }
                let Some(name) = n.name.as_deref() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                if n.kind == llg::core::tokens::TOKEN_GENVAR_DECL {
                    // The lexical Genvar record is the authoritative source
                    // declaration. Remove any elaborated parameter/variable
                    // clone at the same position before the common token path
                    // builds the single index entry.
                    decls.retain(|declaration| {
                        declaration.file != ft.path
                            || declaration.line + 1 != n.line
                            || declaration.col + 1 != n.col
                    });
                    claimed.remove(&(ft.path.clone(), n.line, n.col));
                }
                if claimed.contains(&(ft.path.clone(), n.line, n.col)) {
                    continue;
                }
                let forced_decl =
                    parse_decls.is_some_and(|set| set.contains(&(ft.path.clone(), n.line, n.col)));
                let Some((kind, is_decl)) = classify_token(n.kind, forced_decl) else {
                    continue;
                };
                let len = lsp_name_len(name);
                let line = n.line.saturating_sub(1);
                let col = n.col.saturating_sub(1);
                let scope = enclosing_scope(&scope_providers, &ft.path, n.line);
                let detail = if is_decl {
                    match kind {
                        SymKind::Var if n.kind == llg::core::tokens::TOKEN_GENVAR_DECL => {
                            Some(format!("genvar {name}"))
                        }
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
        let keep: Vec<bool> = decls
            .iter()
            .map(|e| seen.insert((e.file.clone(), e.line, e.col)))
            .collect();
        // `instance_def` is keyed by pre-dedup indices; a dropped duplicate
        // before an instance would otherwise shift its type onto a neighbor.
        let mut new_index = Vec::with_capacity(keep.len());
        let mut kept = 0usize;
        for &keep_entry in &keep {
            new_index.push(keep_entry.then_some(kept));
            kept += usize::from(keep_entry);
        }
        instance_def = instance_def
            .into_iter()
            .filter_map(|(idx, def)| Some((new_index.get(idx).copied().flatten()?, def)))
            .collect();
        let mut keep_iter = keep.into_iter();
        decls.retain(|_| keep_iter.next().unwrap_or(false));
        seen.clear();
        refs.retain(|e| seen.insert((e.file.clone(), e.line, e.col)));

        let mut index = SymbolIndex {
            decls,
            refs,
            instance_def,
            ..SymbolIndex::default()
        };
        index.rebuild_lookup_maps();
        index
    }

    /// The declaration whose name covers the 0-based position.
    pub fn decl_at(&self, file: &str, line: u32, col: u32) -> Option<&SymEntry> {
        let decls = self.decls_in_file(file);
        let after = decls.partition_point(|entry| (entry.line, entry.col) <= (line, col));
        decls[..after]
            .last()
            .filter(|entry| entry.line == line && col < entry.col + lsp_name_len(&entry.name))
    }

    /// Declarations in `file`, sorted by (line, col).
    pub fn decls_in_file(&self, file: &str) -> &[SymEntry] {
        self.decls_by_file.get(file).map_or(&[], |v| v.as_slice())
    }
}

/// Classify a token node as (kind, is_decl); `None` when it is not an indexed
/// symbol site.  See [`SymbolIndex::from_parts`] for the empirical basis.
pub(super) fn classify_token(t: i32, forced_decl: bool) -> Option<(SymKind, bool)> {
    let (slang_kind, slang_declaration) = tokens::token_base_kind(t);
    let slang_symbol = match slang_kind {
        tokens::TOKEN_SLANG_MODULE => Some(SymKind::Module),
        tokens::TOKEN_SLANG_INTERFACE => Some(SymKind::Interface),
        tokens::TOKEN_SLANG_PROGRAM => Some(SymKind::Program),
        tokens::TOKEN_SLANG_PACKAGE => Some(SymKind::Package),
        tokens::TOKEN_SLANG_CLASS => Some(SymKind::Class),
        tokens::TOKEN_SLANG_TYPE_ALIAS
        | tokens::TOKEN_SLANG_STRUCT
        | tokens::TOKEN_SLANG_UNION
        | tokens::TOKEN_SLANG_ENUM => Some(SymKind::Typedef),
        tokens::TOKEN_SLANG_ENUM_MEMBER => Some(SymKind::EnumConst),
        tokens::TOKEN_SLANG_PARAMETER | tokens::TOKEN_SLANG_TYPE_PARAMETER => Some(SymKind::Param),
        tokens::TOKEN_SLANG_PORT => Some(SymKind::Port),
        tokens::TOKEN_SLANG_NET => Some(SymKind::Net),
        tokens::TOKEN_SLANG_VARIABLE
        | tokens::TOKEN_SLANG_FIELD
        | tokens::TOKEN_SLANG_IDENTIFIER
        | tokens::TOKEN_SLANG_BOUND_IDENTIFIER
        | tokens::TOKEN_SLANG_BLOCK => Some(SymKind::Var),
        tokens::TOKEN_SLANG_FUNCTION
        | tokens::TOKEN_SLANG_STATIC_FUNCTION
        | tokens::TOKEN_SLANG_METHOD => Some(SymKind::Function),
        tokens::TOKEN_SLANG_TASK => Some(SymKind::Task),
        tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL
        | tokens::TOKEN_SLANG_PARAMETER_CONNECTION_LABEL => Some(SymKind::Var),
        _ => None,
    };
    if let Some(kind) = slang_symbol {
        return Some((kind, slang_declaration || forced_decl));
    }

    if t == llg::core::tokens::TOKEN_GENVAR_DECL {
        return Some((SymKind::Var, true));
    }
    if t == llg::core::tokens::TOKEN_GENVAR_REF {
        return Some((SymKind::Var, false));
    }

    None
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
