//! Bidirectional pass switches (IEEE 1364-2001 7.6, IEEE 1800-2009 28.8).
//!
//! Each `tran`, `tranif0`, `tranif1`, `rtran`, `rtranif0` or `rtranif1`
//! instance becomes one [`IrSwitch`] between two nodes, each node one bit of
//! a resolved net group. A controlled switch gets a hidden process that sets
//! its state from the control ([`IrStmt::SwitchControl`]); with a delay the
//! state is first an inertial copy of the control on a hidden signal that
//! starts X, so the switch is unknown until the first delay elapses
//! (decision A5-D4). After every process is lowered,
//! [`Codegen::finish_switch_network`] numbers the nodes per statically
//! connected network. Switch terminals keep the structural driver slot the
//! net groups gave them; it is never written, so it stays Z.

use super::*;
use crate::core::db::PrimitiveArrayPosition;
use crate::sim::ir::{IrNetKind, IrSwitch, IrSwitchNetwork, IrSwitchNode};

/// One lowered switch before the networks are numbered.
pub(in super::super) struct PendingSwitch {
    a: IrSwitchNode,
    b: IrSwitchNode,
    resistive: bool,
    controlled: bool,
}

/// Whether a primitive is a bidirectional pass switch.
pub(super) fn is_pass_switch(prim_type: PrimitiveType) -> bool {
    matches!(
        prim_type,
        PrimitiveType::Tran
            | PrimitiveType::Tranif0
            | PrimitiveType::Tranif1
            | PrimitiveType::Rtran
            | PrimitiveType::Rtranif0
            | PrimitiveType::Rtranif1
    )
}

impl Codegen<'_> {
    /// Lower one pass switch instance: resolve its two terminals to nodes and,
    /// for a `tranif`, emit the process that keeps its state.
    pub(super) fn emit_switch(
        &mut self,
        inst: NodeId,
        path: &str,
        g: NodeId,
    ) -> Result<(), String> {
        let NodeKind::Gate {
            prim_type,
            delay,
            terms,
            ..
        } = self.kind(g)
        else {
            unreachable!("non-gate node passed to emit_switch");
        };
        let (prim_type, delay, terms) = (*prim_type, *delay, terms.clone());
        let name = self.node(g).name.clone();
        let shown = if name.is_empty() {
            "switch"
        } else {
            name.as_str()
        };
        let location = self.source_location(g);
        let scope = self.source_path(path);
        let unsupported = |message: String| {
            format!(
                "unsupported bidirectional switch `{shown}` in `{scope}` at {location}: {message}"
            )
        };
        let controlled = !matches!(prim_type, PrimitiveType::Tran | PrimitiveType::Rtran);
        let resistive = matches!(
            prim_type,
            PrimitiveType::Rtran | PrimitiveType::Rtranif0 | PrimitiveType::Rtranif1
        );
        let expected = if controlled { 3 } else { 2 };
        if terms.len() != expected {
            return Err(unsupported(format!(
                "it takes {expected} terminals, not {}",
                terms.len()
            )));
        }
        if !controlled && delay.is_some() {
            return Err(unsupported(
                "`tran` and `rtran` take no delay (IEEE 1800-2009 28.8)".to_owned(),
            ));
        }
        if self.sdf_gate_slot(g).is_some() {
            return Err(unsupported(
                "SDF annotation of a bidirectional switch is not simulated".to_owned(),
            ));
        }
        // An instance array binds each element to the whole terminal
        // expression; the element takes its own bit of it (28.3.6).
        let element = self.db.primitive_array_position(g);
        self.inst = inst;
        let a = self
            .switch_node(terms[0].expr, element)
            .map_err(&unsupported)?;
        let b = self
            .switch_node(terms[1].expr, element)
            .map_err(&unsupported)?;
        if a == b {
            // A switch between a bit and itself changes nothing.
            return Ok(());
        }
        let index = self.pending_switches.len();
        self.pending_switches.push(PendingSwitch {
            a,
            b,
            resistive,
            controlled,
        });
        if !controlled {
            return Ok(());
        }
        let control = self.lower_expr(path, terms[2].expr).map_err(|error| {
            unsupported(format!("the control is not a legal expression: {error}"))
        })?;
        if control.is_real() || control.width() == 0 {
            return Err(unsupported("the control must be a packed value".to_owned()));
        }
        let control = IrExpr::resize_to(control, 1, false);
        // 1 conducts. `ctl ^ 0` and `~ctl` turn a z control into x, so the
        // delayed state never takes a turn-off (z) delay (28.8: the smaller
        // delay applies to x).
        let state = if matches!(prim_type, PrimitiveType::Tranif1 | PrimitiveType::Rtranif1) {
            bin_expr(IrBinOp::BitXor, control, const_bits_expr(1, false))
        } else {
            bitneg_full_width(control)
        };
        let reads = self.collect_read_signals(path, terms[2].expr)?;
        let body = match delay {
            None => vec![IrStmt::SwitchControl {
                switch: index,
                state,
            }],
            Some(delay) => {
                let delay = self
                    .driver_delay(path, g, delay)
                    .map_err(|error| unsupported(format!("its delay: {error}")))?;
                vec![self.delayed_shadow_driver(path, g, state, delay, |read| {
                    vec![IrStmt::SwitchControl {
                        switch: index,
                        state: read,
                    }]
                })?]
            }
        };
        let shape = if reads.is_empty() {
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads }
        };
        let fn_name = self.new_fn_name(path, "switch");
        let origin = self.origin(g);
        self.model.processes.push(IrProcess::new_with_origin(
            fn_name,
            format!("{path}.{shown}"),
            shape,
            Vec::new(),
            body,
            origin,
        ));
        Ok(())
    }

    /// The node a terminal names: a scalar net or a constant bit select of a
    /// packed net (28.8), mapped through inout collapse and true aliases to
    /// its resolved group bit. Wired-logic nets are not admitted.
    ///
    /// For an element of an instance array, a terminal as wide as the array
    /// gives the element its own bit, the least significant one to the
    /// right-hand index (IEEE 1800-2009 28.3.6); a 1-bit terminal connects
    /// to every element. The frontend already slices the control input.
    fn switch_node(
        &self,
        expression: NodeId,
        element: Option<PrimitiveArrayPosition>,
    ) -> Result<IrSwitchNode, String> {
        let mut terminal = self.path_terminal(expression)?;
        let label = self.display_name(terminal.node);
        if !matches!(self.kind(terminal.node), NodeKind::Net { .. }) {
            return Err(format!("terminal `{label}` is not a net"));
        }
        if let Some(element) = element.filter(|_| terminal.width != 1) {
            if terminal.width != element.count {
                return Err(format!(
                    "terminal `{label}` is {} bits wide for an array of {} switches; it must be \
                     1 bit or one bit per switch (IEEE 1800-2009 28.3.6)",
                    terminal.width, element.count
                ));
            }
            terminal.lsb += element.slice;
            terminal.width = 1;
        }
        if terminal.width != 1 {
            return Err(format!(
                "terminal `{label}` is {} bits wide; a terminal is a scalar net or a bit select \
                 (IEEE 1800-2009 28.8)",
                terminal.width
            ));
        }
        let signal = self.model.signal(terminal.info.ir);
        let node = if let Some(binding) = signal
            .net_alias
            .iter()
            .find(|binding| binding.signal_bit() == terminal.lsb)
        {
            IrSwitchNode::new(binding.group(), binding.group_bit())
        } else if let Some((group, _)) = signal.net_driver {
            if self.model.net_groups[group].width != terminal.info.width {
                return Err(format!(
                    "terminal `{label}` shares storage with a wider network"
                ));
            }
            IrSwitchNode::new(group, terminal.lsb)
        } else {
            return Err(format!("terminal `{label}` has no resolved net storage"));
        };
        if matches!(
            self.model.net_groups[node.group()].kind,
            IrNetKind::Wand | IrNetKind::Wor
        ) {
            return Err(format!(
                "terminal `{label}` is a wired-logic (wand, wor, triand or trior) net; \
                 wired resolution across a switch network is not simulated"
            ));
        }
        Ok(node)
    }

    /// Number the nodes of every statically connected switch network
    /// contiguously, in order of first use, and store the network in the
    /// model. Switch indices keep their lowering order, which the control
    /// processes already name.
    pub(in super::super) fn finish_switch_network(&mut self) -> Result<(), String> {
        let pending = std::mem::take(&mut self.pending_switches);
        if pending.is_empty() {
            return Ok(());
        }
        let mut keys: Vec<IrSwitchNode> = Vec::new();
        let mut key_index: HashMap<IrSwitchNode, usize> = HashMap::new();
        let mut intern = |node: IrSwitchNode, keys: &mut Vec<IrSwitchNode>| {
            *key_index.entry(node).or_insert_with(|| {
                keys.push(node);
                keys.len() - 1
            })
        };
        let ends = pending
            .iter()
            .map(|switch| (intern(switch.a, &mut keys), intern(switch.b, &mut keys)))
            .collect::<Vec<_>>();
        // Union-find over the nodes with path halving.
        let mut parent = (0..keys.len()).collect::<Vec<_>>();
        fn find(parent: &mut [usize], mut node: usize) -> usize {
            while parent[node] != node {
                parent[node] = parent[parent[node]];
                node = parent[node];
            }
            node
        }
        for &(a, b) in &ends {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
        // Networks in order of their first node; nodes in first-use order.
        let mut members: Vec<Vec<usize>> = Vec::new();
        let mut network_of_root: HashMap<usize, usize> = HashMap::new();
        for node in 0..keys.len() {
            let root = find(&mut parent, node);
            let network = *network_of_root.entry(root).or_insert_with(|| {
                members.push(Vec::new());
                members.len() - 1
            });
            members[network].push(node);
        }
        let mut number = vec![0u32; keys.len()];
        let mut nodes = Vec::with_capacity(keys.len());
        let mut components = Vec::with_capacity(members.len());
        let index =
            |value: usize| u32::try_from(value).map_err(|_| "too many switch nodes".to_owned());
        for network in &members {
            components.push(index(nodes.len())?);
            for &node in network {
                number[node] = index(nodes.len())?;
                nodes.push(keys[node]);
            }
        }
        let switches = pending
            .iter()
            .zip(&ends)
            .map(|(switch, &(a, b))| {
                IrSwitch::new(number[a], number[b], switch.resistive, switch.controlled)
            })
            .collect();
        self.model.switch_network = IrSwitchNetwork::new(nodes, switches, components);
        Ok(())
    }
}
