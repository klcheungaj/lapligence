//! The declarations of blocks and loops, looked up by name from the nodes
//! inside them.

use super::super::nesting::Enclosing;
use super::*;

/// The declarations of every block and loop, by the name they declare: a
/// lookup passes neither the scopes that do not declare the name nor the other
/// statements of those that do.
pub(in super::super) struct LexicalScopes {
    declarations: HashMap<String, Enclosing<Vec<NodeId>>>,
    /// Every `for` statement.
    loops: Enclosing<()>,
}

impl LexicalScopes {
    fn new(cg: &Codegen<'_>) -> Self {
        let mut declarations = HashMap::<&str, Vec<(NodeId, Vec<NodeId>)>>::new();
        let mut loops = Vec::new();
        for scope in cg.db.node_ids() {
            let declare = |variable: NodeId| {
                let scopes = declarations
                    .entry(cg.node(variable).name.as_str())
                    .or_default();
                match scopes.last_mut() {
                    Some((last, variables)) if *last == scope => variables.push(variable),
                    _ => scopes.push((scope, vec![variable])),
                }
            };
            match cg.kind(scope) {
                NodeKind::Stmt(StmtKind::Begin) => cg
                    .node(scope)
                    .children
                    .iter()
                    .copied()
                    .filter(|child| !matches!(cg.kind(*child), NodeKind::Stmt(_)))
                    .for_each(declare),
                NodeKind::Stmt(StmtKind::For { vars, .. }) => {
                    loops.push((scope, ()));
                    vars.iter().copied().for_each(declare);
                }
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => {
                    vars.iter().flatten().copied().for_each(declare);
                }
                _ => {}
            }
        }
        Self {
            declarations: declarations
                .into_iter()
                .map(|(name, scopes)| (name.to_owned(), Enclosing::new(cg.nesting(), scopes)))
                .collect(),
            loops: Enclosing::new(cg.nesting(), loops),
        }
    }
}

impl Codegen<'_> {
    fn lexical_scopes(&self) -> &LexicalScopes {
        self.lexical_scopes.get_or_init(|| LexicalScopes::new(self))
    }

    /// The blocks and loops around `reference` that declare its name, the
    /// innermost first, with their declarations of it.
    pub(super) fn lexical_declarations(
        &self,
        reference: NodeId,
    ) -> impl Iterator<Item = (NodeId, &[NodeId])> + '_ {
        self.lexical_scopes()
            .declarations
            .get(self.node(reference).name.as_str())
            .into_iter()
            .flat_map(move |scopes| scopes.around(self.nesting(), reference))
            .map(|(scope, variables)| (scope, variables.as_slice()))
    }

    /// The declarations in `scope` that are variables: a block has other
    /// members.
    pub(super) fn lexical_variables<'s>(
        &'s self,
        scope: NodeId,
        variables: &'s [NodeId],
    ) -> impl Iterator<Item = NodeId> + 's {
        let block = matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin));
        variables
            .iter()
            .copied()
            .filter(move |variable| !block || matches!(self.kind(*variable), NodeKind::Var { .. }))
    }

    /// The innermost `for` statement around `node`.
    pub(super) fn innermost_lexical_loop(&self, node: NodeId) -> Option<NodeId> {
        self.lexical_scopes()
            .loops
            .around(self.nesting(), node)
            .next()
            .map(|(statement, ())| statement)
    }

    /// What a lookup of the name of `node` in the blocks and loops around it
    /// depends on, unless all that could be found is inside `within`.
    pub(super) fn lexical_key(
        &self,
        node: NodeId,
        within: NodeId,
    ) -> Option<(&str, NodeId, Option<NodeId>)> {
        let (innermost, _) = self.lexical_declarations(node).next()?;
        self.lexical_declarations(node)
            .flat_map(|(_, variables)| variables)
            .any(|variable| !self.node_is_within(*variable, within))
            .then(|| {
                (
                    self.node(node).name.as_str(),
                    innermost,
                    self.innermost_lexical_loop(node),
                )
            })
    }
}
