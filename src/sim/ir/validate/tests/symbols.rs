//! Every file-scope symbol of the model has its own name.
use super::*;

fn function(name: &str, pre_fns: Vec<IrPreFn>) -> IrFunc {
    IrFunc::new(name.into(), None, vec![], vec![], pre_fns, vec![])
}

fn process(name: &str, pre_fns: Vec<IrPreFn>) -> IrProcess {
    IrProcess::new(name.into(), name.into(), IrShape::RunOnce, pre_fns, vec![])
}

fn branch(name: &str) -> IrPreFn {
    IrPreFn::Branch {
        c_name: name.into(),
        body: vec![],
    }
}

/// A model whose `index`th symbol is named `name`, the others by position.
fn model_with(index: usize, name: &str) -> IrModel {
    let mut next = 0;
    let mut symbol = || {
        next += 1;
        if next - 1 == index {
            name.to_owned()
        } else {
            format!("symbol_{}", next - 1)
        }
    };
    let mut model = valid_model();
    model.signals[0].c_name = symbol();
    let mut other = model.signals[0].clone();
    other.c_name = symbol();
    model.signals.push(other);
    for _ in 0..2 {
        model
            .arrays
            .push(IrArray::new(symbol(), "a".into(), 1, false, vec![(0, 1)]).unwrap());
        model
            .net_groups
            .push(IrNetGroup::new(symbol(), 1, false, IrNetKind::Wire, 1).unwrap());
        model.containers.push(IrContainer {
            c_name: symbol(),
            element: IrContainerElement::Real { shortreal: false },
            kind: IrContainerKind::Dynamic,
            initial_size: None,
            activation: false,
            class_field: None,
            receiver: None,
        });
        model.objects.push(IrObject {
            c_name: symbol(),
            ty: IrObjectType::String,
            initial: None,
        });
        model.events.push(crate::sim::ir::IrEvent::new(symbol()));
        model.native_values.push(IrNativeValue {
            c_name: symbol(),
            ty: 0,
            activation: false,
            companions: vec![],
            class_field: None,
            receiver: None,
            marker: false,
            ref_rooted: false,
        });
        model
            .funcs
            .push(function(&symbol(), vec![branch(&symbol())]));
        model
            .processes
            .push(process(&symbol(), vec![branch(&symbol())]));
    }
    model.events.push(crate::sim::ir::IrEvent::new_array(
        symbol(),
        vec![(0, 0)],
        vec![0],
    ));
    model.native_types.push(IrContainerElement::Aggregate {
        type_id: 3,
        members: vec![IrContainerMember {
            name: "r".into(),
            element: Box::new(IrContainerElement::Real { shortreal: false }),
        }],
    });
    model
}

const SYMBOLS: usize = 23;

#[test]
fn symbol_names_are_one_name_space_over_every_kind() {
    model_with(SYMBOLS, "").validate().unwrap();
    for taken in 0..SYMBOLS {
        let name = format!("symbol_{taken}");
        for index in (0..SYMBOLS).filter(|index| *index != taken) {
            let error = model_with(index, &name)
                .validate()
                .expect_err("two symbols share a name");
            assert!(
                error
                    .to_string()
                    .contains(&format!("name `{name}` is not unique")),
                "{index} as {taken}: {error}"
            );
        }
    }
}

#[test]
fn storage_without_a_symbol_of_its_own_may_repeat_a_name() {
    let mut model = model_with(SYMBOLS, "");
    let mut alias = model.signals[0].clone();
    alias.alias = Some(0);
    model.signals.push(alias);
    let mut omitted = model.signals[0].clone();
    omitted.omit = true;
    model.signals.push(omitted);
    let mut activation = model.arrays[0].clone();
    activation.activation = true;
    model.arrays.push(activation);
    model.validate().unwrap();
}
