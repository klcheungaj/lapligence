//! Class virtual dispatch tables.

use std::collections::{BTreeMap, HashMap};

use super::{IrClass, IrFunc, IrModel};

/// The virtual methods of a model, indexed once by slot, so that no consumer
/// scans the functions per class, slot or call site.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::sim) struct VirtualDispatch {
    slots: BTreeMap<usize, SlotMethods>,
    /// The direct subclasses of each class, ascending.
    subclasses: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SlotMethods {
    /// Every function with the slot, ascending.
    functions: Vec<usize>,
    /// The first function each class declares for the slot.
    declared: HashMap<usize, usize>,
}

impl VirtualDispatch {
    pub(in crate::sim) fn new(model: &IrModel) -> Self {
        Self::of(&model.funcs, &model.classes)
    }

    fn of(funcs: &[IrFunc], classes: &[IrClass]) -> Self {
        let mut slots = BTreeMap::<usize, SlotMethods>::new();
        for (index, function) in funcs.iter().enumerate() {
            let Some(slot) = function.virtual_slot else {
                continue;
            };
            let methods = slots.entry(slot).or_default();
            methods.functions.push(index);
            if let Some(class) = function.receiver_class {
                methods.declared.entry(class).or_insert(index);
            }
        }
        let mut subclasses = vec![Vec::new(); classes.len()];
        for (index, class) in classes.iter().enumerate() {
            if let Some(derived) = class.base.and_then(|base| subclasses.get_mut(base)) {
                derived.push(index);
            }
        }
        Self { slots, subclasses }
    }

    /// Each slot with its first function, which gives the slot its signature,
    /// in slot order.
    pub(in crate::sim) fn slots(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.slots
            .iter()
            .filter_map(|(slot, methods)| Some((*slot, *methods.functions.first()?)))
    }

    /// Every function with the slot, ascending.
    pub(in crate::sim) fn functions(&self, slot: usize) -> &[usize] {
        self.slots
            .get(&slot)
            .map_or(&[], |methods| methods.functions.as_slice())
    }

    /// The classes that respond to a slot, ascending, each with the function
    /// it runs: the one the class declares or its nearest base's. An
    /// inline-expanded template has no body: calls reach its specializations
    /// through a family slot of their own (`record_ref_family`), so a class
    /// that would run one is left out.
    pub(in crate::sim) fn implementations(
        &self,
        model: &IrModel,
        slot: usize,
    ) -> Vec<(usize, usize)> {
        let Some(methods) = self.slots.get(&slot) else {
            return Vec::new();
        };
        let mut implementations = Vec::new();
        let mut pending = Vec::new();
        for (&class, &function) in &methods.declared {
            if class >= self.subclasses.len() || model.funcs[function].inline_expanded {
                continue;
            }
            pending.push(class);
            while let Some(class) = pending.pop() {
                count_visit();
                implementations.push((class, function));
                pending.extend(
                    self.subclasses[class]
                        .iter()
                        .filter(|derived| !methods.declared.contains_key(derived)),
                );
            }
        }
        implementations.sort_unstable();
        implementations
    }
}

#[cfg(test)]
thread_local! {
    static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Count a class that a dispatch query visits.
#[inline]
fn count_visit() {
    #[cfg(test)]
    VISITS.with(|visits| visits.set(visits.get() + 1));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(receiver class, slot, template)` of each function.
    type Method = (usize, usize, bool);

    fn dispatch(bases: &[Option<usize>], methods: &[Method]) -> (IrModel, VirtualDispatch) {
        let mut model = IrModel::new("dispatch".to_owned(), 1_000_000).unwrap();
        model.classes = bases
            .iter()
            .enumerate()
            .map(|(index, base)| IrClass {
                c_name: format!("class_{index}"),
                base: *base,
                fields: Vec::new(),
                rank: 0,
            })
            .collect();
        for (index, &(class, slot, template)) in methods.iter().enumerate() {
            let mut function = IrFunc::new(
                format!("method_{index}"),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            function.receiver_class = Some(class);
            function.virtual_slot = Some(slot);
            function.inline_expanded = template;
            model.funcs.push(function);
        }
        let dispatch = VirtualDispatch::new(&model);
        (model, dispatch)
    }

    /// The definition of dispatch, by scanning: the first function with the
    /// slot in the nearest class of the base chain.
    fn scanned(bases: &[Option<usize>], methods: &[Method], slot: usize) -> Vec<(usize, usize)> {
        (0..bases.len())
            .filter_map(|class| {
                let mut current = Some(class);
                while let Some(owner) = current {
                    if let Some(index) = methods
                        .iter()
                        .position(|method| (method.0, method.1) == (owner, slot))
                    {
                        return (!methods[index].2).then_some((class, index));
                    }
                    current = bases[owner];
                }
                None
            })
            .collect()
    }

    #[test]
    fn a_class_runs_its_own_method_or_its_nearest_bases() {
        // Two trees; class 2 is declared before its base.
        let bases = [None, Some(0), Some(4), Some(1), Some(1), None, Some(5)];
        let methods = [
            (0, 0, false),
            (4, 0, false),
            (2, 1, false),
            // A second method of class 4 with the slot never runs.
            (4, 0, false),
            (5, 2, false),
            (6, 2, true),
            (1, 3, true),
            (3, 3, false),
        ];
        let (model, dispatch) = dispatch(&bases, &methods);
        for slot in 0..5 {
            assert_eq!(
                dispatch.implementations(&model, slot),
                scanned(&bases, &methods, slot),
                "slot {slot}"
            );
        }
        assert_eq!(
            dispatch.implementations(&model, 0),
            [(0, 0), (1, 0), (2, 1), (3, 0), (4, 1)]
        );
        assert_eq!(dispatch.functions(0), [0, 1, 3]);
        assert!(dispatch.functions(5).is_empty());
        assert_eq!(
            dispatch.slots().collect::<Vec<_>>(),
            [(0, 0), (1, 2), (2, 4), (3, 6)]
        );
    }

    #[test]
    fn a_query_visits_only_the_classes_that_respond() {
        const CLASSES: usize = 400;
        // Each class declares a slot of its own and overrides slot 0.
        let bases: Vec<_> = (0..CLASSES)
            .map(|class| class.checked_sub(1).map(|base| base / 4))
            .collect();
        let methods: Vec<_> = (0..CLASSES)
            .flat_map(|class| [(class, 0, false), (class, class + 1, false)])
            .collect();
        let (model, dispatch) = dispatch(&bases, &methods);
        VISITS.with(|visits| visits.set(0));
        let mut cases = 0;
        for (slot, _) in dispatch.slots() {
            let implementations = dispatch.implementations(&model, slot);
            assert_eq!(implementations, scanned(&bases, &methods, slot));
            cases += implementations.len();
        }
        assert_eq!(VISITS.with(std::cell::Cell::get), cases);
    }
}
