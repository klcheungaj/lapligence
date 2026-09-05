use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use super::{Db, NodeId, NodeKind};

/// A structural invariant violation in an owned [`Db`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbValidationError {
    path: String,
    detail: String,
}

impl DbValidationError {
    fn new(path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            detail: detail.into(),
        }
    }

    /// Location of the invalid value within the database.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Human-readable description of the violated invariant.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for DbValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid owned UHDM database at {}: {}",
            self.path, self.detail
        )
    }
}

impl Error for DbValidationError {}

impl Db {
    /// Validate arena links and side-table references after VPI capture.
    pub fn validate(&self) -> Result<(), DbValidationError> {
        let validator = Validator { db: self };
        validator.validate()
    }
}

struct Validator<'db> {
    db: &'db Db,
}

impl Validator<'_> {
    fn validate(&self) -> Result<(), DbValidationError> {
        for (index, node) in self.db.nodes.iter().enumerate() {
            let path = format!("nodes[{index}]");
            if let Some(parent) = node.parent {
                self.node(parent, &format!("{path}.parent"))?;
            }
            for (child_index, child) in node.children.iter().copied().enumerate() {
                // `children` includes both owned descendants and resolved
                // cross-references (for example port connections), so only
                // arena validity is universal here; reciprocity is not.
                self.node(child, &format!("{path}.children[{child_index}]"))?;
            }
        }

        self.validate_roots(&self.db.tops, "tops", |kind| {
            matches!(kind, NodeKind::ModuleInst { is_top: true, .. })
        })?;
        self.validate_roots(&self.db.flat_modules, "flat_modules", |kind| {
            matches!(kind, NodeKind::ModuleInst { .. })
        })?;
        self.validate_roots(&self.db.packages, "packages", |kind| {
            matches!(kind, NodeKind::Package)
        })?;
        self.validate_roots(&self.db.classes, "classes", |kind| {
            matches!(kind, NodeKind::ClassDef)
        })?;

        for (array, metadata) in &self.db.arrays {
            let node = self.node(*array, &format!("arrays[{}]", array.0))?;
            if !matches!(node.kind, NodeKind::Array { .. }) {
                return self.fail(
                    format!("arrays[{}]", array.0),
                    "metadata key is not an array node",
                );
            }
            if let Some(init) = metadata.init {
                self.node(init, &format!("arrays[{}].init", array.0))?;
            }
        }

        for (variable, init) in &self.db.vars_init {
            let node = self.node(*variable, &format!("vars_init[{}]", variable.0))?;
            if !matches!(node.kind, NodeKind::Var { .. }) {
                return self.fail(
                    format!("vars_init[{}]", variable.0),
                    "initializer key is not a variable node",
                );
            }
            self.node(*init, &format!("vars_init[{}]", variable.0))?;
        }

        let mut range_keys = HashSet::new();
        for (index, entry) in self.db.elaborated_type_ranges.iter().enumerate() {
            if entry.instance.is_empty() || entry.name.is_empty() {
                return self.fail(
                    format!("elaborated_type_ranges[{index}]"),
                    "instance and object names must be non-empty",
                );
            }
            if !range_keys.insert((entry.instance.as_str(), entry.name.as_str())) {
                return self.fail(
                    format!("elaborated_type_ranges[{index}]"),
                    "duplicate instance/object range entry",
                );
            }
        }
        Ok(())
    }

    fn validate_roots(
        &self,
        roots: &[NodeId],
        path: &str,
        expected_kind: impl Fn(&NodeKind) -> bool,
    ) -> Result<(), DbValidationError> {
        let mut seen = HashSet::new();
        for (index, root) in roots.iter().copied().enumerate() {
            if !seen.insert(root) {
                return self.fail(
                    format!("{path}[{index}]"),
                    format!("node {} is listed more than once", root.0),
                );
            }
            let node = self.node(root, &format!("{path}[{index}]"))?;
            if node.parent.is_some() {
                return self.fail(format!("{path}[{index}]"), "root node has a parent");
            }
            if !expected_kind(&node.kind) {
                return self.fail(format!("{path}[{index}]"), "root has the wrong node kind");
            }
        }
        Ok(())
    }

    fn node(&self, id: NodeId, path: &str) -> Result<&super::Node, DbValidationError> {
        self.db.nodes.get(id.0 as usize).ok_or_else(|| {
            DbValidationError::new(path, format!("node index {} is out of bounds", id.0))
        })
    }

    fn fail<T>(
        &self,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<T, DbValidationError> {
        Err(DbValidationError::new(path, detail))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn empty_db() -> Db {
        Db {
            nodes: Vec::new(),
            tops: Vec::new(),
            flat_modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
            design_name: "test".to_string(),
            arrays: HashMap::new(),
            vars_init: HashMap::new(),
            elaborated_type_ranges: Vec::new(),
        }
    }

    #[test]
    fn rejects_an_out_of_bounds_root() {
        let mut db = empty_db();
        db.tops.push(NodeId(0));

        let error = db.validate().expect_err("invalid root must fail");
        assert_eq!(error.path(), "tops[0]");
        assert!(error.detail().contains("out of bounds"));
    }

    #[test]
    fn accepts_an_empty_structural_snapshot() {
        empty_db()
            .validate()
            .expect("empty graph is structurally valid");
    }
}
