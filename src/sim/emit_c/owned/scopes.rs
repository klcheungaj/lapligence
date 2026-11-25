//! Names bound in nested lexical scopes.

use std::collections::HashMap;
use std::rc::Rc;

/// The bindings of nested scopes: an inner one hides the outer ones of its
/// name. A lookup does not depend on how deep the scopes nest.
pub(super) struct Scopes<V> {
    bound: HashMap<Rc<str>, Bound<V>>,
    /// The names each scope binds, the outermost scope first.
    scopes: Vec<Vec<Rc<str>>>,
}

/// The bindings of one name, with the index of the scope of each.
struct Bound<V> {
    innermost: (usize, V),
    /// The hidden ones, outermost first.
    outer: Vec<(usize, V)>,
}

impl<V> Scopes<V> {
    /// One scope, the outermost.
    pub(super) fn new() -> Self {
        Self {
            bound: HashMap::new(),
            scopes: vec![Vec::new()],
        }
    }

    pub(super) fn push(&mut self) {
        self.scopes.push(Vec::new());
    }

    /// Leave the innermost scope.
    pub(super) fn pop(&mut self) {
        let Some(names) = self.scopes.pop() else {
            return;
        };
        for name in names {
            self.unbind(&name, self.scopes.len());
        }
    }

    /// The innermost binding of `name`.
    pub(super) fn get(&self, name: &str) -> Option<&V> {
        self.bound.get(name).map(|bound| &bound.innermost.1)
    }

    pub(super) fn contains(&self, name: &str) -> bool {
        self.bound.contains_key(name)
    }

    /// The binding of `name` in the innermost scope itself.
    pub(super) fn get_innermost(&self, name: &str) -> Option<&V> {
        let bound = self.bound.get(name)?;
        (bound.innermost.0 + 1 == self.scopes.len()).then_some(&bound.innermost.1)
    }

    pub(super) fn get_innermost_mut(&mut self, name: &str) -> Option<&mut V> {
        let bound = self.bound.get_mut(name)?;
        (bound.innermost.0 + 1 == self.scopes.len()).then_some(&mut bound.innermost.1)
    }

    /// The binding of `name` in the outermost scope.
    pub(super) fn get_outermost(&self, name: &str) -> Option<&V> {
        let bound = self.bound.get(name)?;
        let (scope, value) = bound.outer.first().unwrap_or(&bound.innermost);
        (*scope == 0).then_some(value)
    }

    pub(super) fn get_outermost_mut(&mut self, name: &str) -> Option<&mut V> {
        let bound = self.bound.get_mut(name)?;
        let (scope, value) = bound.outer.first_mut().unwrap_or(&mut bound.innermost);
        (*scope == 0).then_some(value)
    }

    /// Bind `name` in the innermost scope.
    pub(super) fn insert(&mut self, name: &str, value: V) {
        let scope = self.scopes.len() - 1;
        if let Some(bound) = self.bound.get_mut(name) {
            let replaced = std::mem::replace(&mut bound.innermost, (scope, value));
            if replaced.0 == scope {
                return;
            }
            bound.outer.push(replaced);
            let name = self.name(name);
            self.scopes[scope].push(name);
            return;
        }
        self.insert_new(name, scope, value);
    }

    /// Bind `name` in the outermost scope.
    pub(super) fn insert_outermost(&mut self, name: &str, value: V) {
        if let Some(existing) = self.get_outermost_mut(name) {
            *existing = value;
            return;
        }
        if let Some(bound) = self.bound.get_mut(name) {
            bound.outer.insert(0, (0, value));
            let name = self.name(name);
            self.scopes[0].push(name);
            return;
        }
        self.insert_new(name, 0, value);
    }

    /// Unbind `name` in the innermost scope.
    pub(super) fn remove(&mut self, name: &str) {
        self.unbind(name, self.scopes.len() - 1);
    }

    /// The shared spelling of a bound name.
    fn name(&self, name: &str) -> Rc<str> {
        match self.bound.get_key_value(name) {
            Some((name, _)) => name.clone(),
            None => name.into(),
        }
    }

    fn insert_new(&mut self, name: &str, scope: usize, value: V) {
        let name = Rc::<str>::from(name);
        self.scopes[scope].push(name.clone());
        self.bound.insert(
            name,
            Bound {
                innermost: (scope, value),
                outer: Vec::new(),
            },
        );
    }

    fn unbind(&mut self, name: &str, scope: usize) {
        let Some(bound) = self.bound.get_mut(name) else {
            return;
        };
        if bound.innermost.0 != scope {
            return;
        }
        match bound.outer.pop() {
            Some(outer) => bound.innermost = outer,
            None => {
                self.bound.remove(name);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn depth(&self) -> usize {
        self.scopes.len()
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.bound.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inner_binding_hides_the_outer_one_until_its_scope_ends() {
        let mut scopes = Scopes::new();
        scopes.insert("a", 1);
        scopes.push();
        assert_eq!(scopes.get("a"), Some(&1));
        assert_eq!(scopes.get_innermost("a"), None);
        scopes.insert("a", 2);
        scopes.insert("a", 3);
        scopes.insert("b", 4);
        assert_eq!(scopes.get("a"), Some(&3));
        assert_eq!(scopes.get_outermost("a"), Some(&1));
        assert_eq!(scopes.get_outermost("b"), None);
        scopes.push();
        scopes.push();
        scopes.insert("a", 5);
        scopes.pop();
        assert_eq!(scopes.get("a"), Some(&3));
        scopes.pop();
        assert_eq!(scopes.get_innermost("a"), Some(&3));
        scopes.pop();
        assert_eq!(scopes.get("a"), Some(&1));
        assert!(!scopes.contains("b"));
    }

    #[test]
    fn the_outermost_scope_binds_below_inner_ones() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.insert("a", 2);
        scopes.insert_outermost("a", 1);
        scopes.insert_outermost("b", 3);
        assert_eq!(scopes.get("a"), Some(&2));
        assert_eq!(scopes.get_outermost("a"), Some(&1));
        scopes.remove("a");
        assert_eq!(scopes.get("a"), Some(&1));
        scopes.remove("a");
        assert_eq!(scopes.get("a"), Some(&1));
        scopes.insert("b", 4);
        scopes.pop();
        assert_eq!((scopes.get("a"), scopes.get("b")), (Some(&1), Some(&3)));
    }
}
