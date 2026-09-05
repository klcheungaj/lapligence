use crate::core::elab::Val;
use crate::ffi::vpi::{self, VpiHandle};
use std::collections::{HashMap, HashSet};

use super::super::database::*;

impl Builder {
    pub(in crate::core::db) fn walk_module_inst(
        &mut self,
        h: VpiHandle,
        parent_file: Option<&str>,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let mut props = self.common(h);
        let def_name = vpi::get_str(vpi::vpiDefName, h);
        let is_top = parent.is_none();
        let is_interface = vpi::obj_type(h) == vpi::vpiInterface;
        let timeunit = vpi::get(vpi::vpiTimeUnit, h);
        let timeprecision = vpi::get(vpi::vpiTimePrecision, h);
        // Top instances have no vpiFullName; the name (the def name) is used.
        // Empty files inherit the parent instance's file.
        if props.full_name.is_empty() {
            props.full_name = props.name.clone();
        }
        if props.file.is_empty() {
            props.file = parent_file.map(str::to_string).unwrap_or_default();
        }
        let id = self.register(
            parent,
            &props,
            NodeKind::ModuleInst {
                def_name,
                is_top,
                is_interface,
                timeunit,
                timeprecision,
            },
        );
        self.index_node(h, &props, id);

        let mut kids: Vec<NodeId> = Vec::new();
        // Unpacked arrays declare a module-level "element" net/var with the
        // array's own name (a Surelog elaboration quirk: the element object is
        // reported both as a plain net under `vpiNet` and as the array's
        // `vpiNet`/`vpiReg` child).  Collect the array full names first so
        // those duplicate element nets are skipped below.
        let mut array_names: HashSet<String> = HashSet::new();
        for v in iter(vpi::vpiVariables, h) {
            let v = v.raw();
            if is_array_type(vpi::obj_type(v)) {
                let full = vpi::obj_full_name(v);
                if !full.is_empty() {
                    array_names.insert(full);
                }
            }
        }
        for a in iter(vpi::vpiArrayNet, h) {
            let a = a.raw();
            let full = vpi::obj_full_name(a);
            if !full.is_empty() {
                array_names.insert(full);
            }
        }
        // Nets/vars/params are indexed before everything that references them
        // (port connections, expression operands, modport io_decls).
        for net in iter(vpi::vpiNet, h) {
            let net = net.raw();
            let full = vpi::obj_full_name(net);
            if array_names.contains(&full) {
                continue; // element net of an unpacked array
            }
            self.capture_elaborated_type_ranges(h, net);
            kids.push(self.walk_net(net, Some(id))?);
        }
        for var in iter(vpi::vpiVariables, h) {
            let var = var.raw();
            if is_array_type(vpi::obj_type(var)) {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_array(var, Some(id), false)?);
            } else {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_var(var, Some(id))?);
            }
        }
        for arr in iter(vpi::vpiArrayNet, h) {
            let arr = arr.raw();
            self.capture_elaborated_type_ranges(h, arr);
            kids.push(self.walk_array(arr, Some(id), true)?);
        }
        // Enum constants declared by module-local typedefs must be captured
        // before process expressions that refer to them.  Surelog exposes
        // the declarations beneath each enum `vpiTypedef`, while uses are
        // ref objects whose `vpiActual` points back to those constants.
        for ts in iter(vpi::vpiTypedef, h) {
            let ts = ts.raw();
            if vpi::obj_type(ts) != vpi::vpiEnumTypespec {
                continue;
            }
            for ec in iter(vpi::vpiEnumConst, ts) {
                kids.push(self.walk_enum_const(ec.raw(), Some(id))?);
            }
        }
        // Named events (`event ev;`) are indexed before anything that
        // references them (trigger statements inside process bodies resolve
        // by name against these captures).
        for ne in iter(vpi::vpiNamedEvent, h) {
            kids.push(self.walk_named_event(ne.raw(), Some(id))?);
        }
        // Functions/tasks are captured before anything that calls them
        // (param_assign RHS calls, process bodies, nested function bodies),
        // so `FuncCall.callee` resolution finds the per-instance clone.
        for tf in iter(vpi::vpiTaskFunc, h) {
            kids.push(self.walk_task_func(tf.raw(), Some(id))?);
        }
        // Parameters: resolved values; a resolution failure leaves every
        // parameter of this instance with `value: None` (never fails).
        let resolved = self
            .resolver
            .scope_params(h)
            .unwrap_or_default()
            .into_iter()
            .collect::<HashMap<String, Val>>();
        for p in iter(vpi::vpiParameter, h) {
            let p = p.raw();
            let value = resolved.get(&vpi::obj_name(p)).cloned();
            self.capture_elaborated_type_ranges(h, p);
            kids.push(self.walk_param(p, Some(id), value)?);
        }
        // Modports (interface instances only) are indexed before ports so
        // interface port `low` connections resolve to the copy's modport.
        for mp in iter(vpi::vpiModport, h) {
            kids.push(self.walk_modport(mp.raw(), Some(id))?);
        }
        // Child instances come before ports: an interface port's `low` (the
        // per-port copy) and a plain port's `low` (the child-side signal)
        // live inside the child and must be captured first.
        for iface in iter(vpi::vpiInterface, h) {
            kids.push(self.walk_module_inst(iface.raw(), Some(props.file.as_str()), Some(id))?);
        }
        for c in iter(vpi::vpiModule, h) {
            kids.push(self.walk_module_inst(c.raw(), Some(props.file.as_str()), Some(id))?);
        }
        for port in iter(vpi::vpiPort, h) {
            let port = port.raw();
            self.capture_elaborated_type_ranges(h, port);
            kids.push(self.walk_port(port, Some(id))?);
        }
        for pa in iter(vpi::vpiParamAssign, h) {
            kids.push(self.walk_param_assign(pa.raw(), Some(id))?);
        }
        for proc in iter(vpi::vpiProcess, h) {
            kids.push(self.walk_process(proc.raw(), Some(id))?);
        }
        for ca in iter(vpi::vpiContAssign, h) {
            kids.push(self.walk_cont_assign(ca.raw(), Some(id))?);
        }
        // Structural primitives (gates, enable gates, pullup/pulldown,
        // switch/transistor primitives, UDP instances) and their arrays, in
        // document order after the continuous assignments.  Gate terminals
        // reference nets/vars captured above.
        for p in iter(vpi::vpiPrimitive, h) {
            kids.push(self.walk_primitive(p.raw(), Some(id))?);
        }
        for pa in iter(vpi::vpiPrimitiveArray, h) {
            kids.push(self.walk_primitive_array(pa.raw(), Some(id))?);
        }
        for gsa in iter(vpi::vpiGenScopeArray, h) {
            kids.push(self.walk_gen_scope_array(gsa.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_flat_module(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let def_name = vpi::get_str(vpi::vpiDefName, h);
        let timeunit = vpi::get(vpi::vpiTimeUnit, h);
        let timeprecision = vpi::get(vpi::vpiTimePrecision, h);
        let id = self.register(
            parent,
            &props,
            NodeKind::ModuleInst {
                def_name,
                is_top: false,
                is_interface: false,
                timeunit,
                timeprecision,
            },
        );
        let mut kids = Vec::new();
        for ts in iter(vpi::vpiTypedef, h) {
            let ts = ts.raw();
            if vpi::obj_type(ts) != vpi::vpiEnumTypespec {
                continue;
            }
            for ec in iter(vpi::vpiEnumConst, ts) {
                kids.push(self.walk_enum_const(ec.raw(), Some(id))?);
            }
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_package(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::Package);
        let mut kids: Vec<NodeId> = Vec::new();
        // Parameters: resolved values; a resolution failure leaves every
        // parameter of this package with `value: None` (never fails).
        let resolved = match self.resolver.scope_params(h) {
            Ok(v) => v.into_iter().collect::<HashMap<String, Val>>(),
            Err(_) => HashMap::new(),
        };
        for p in iter(vpi::vpiParameter, h) {
            let p = p.raw();
            let value = resolved.get(&vpi::obj_name(p)).cloned();
            kids.push(self.walk_param(p, Some(id), value)?);
        }
        // Enum constants live under the package's `vpiTypedef` children
        // (Surelog v1.86 emits each enum typedef as an `enum_typespec` with
        // one `vpiEnumConst` per enumerator, in declaration order).
        for ts in iter(vpi::vpiTypedef, h) {
            let ts = ts.raw();
            if vpi::obj_type(ts) != vpi::vpiEnumTypespec {
                continue;
            }
            for ec in iter(vpi::vpiEnumConst, ts) {
                kids.push(self.walk_enum_const(ec.raw(), Some(id))?);
            }
        }
        // Functions/tasks declared directly in the package.
        for tf in iter(vpi::vpiTaskFunc, h) {
            kids.push(self.walk_task_func(tf.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_class_defn(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::ClassDef);
        self.index_node(h, &props, id);
        let mut kids: Vec<NodeId> = Vec::new();
        for v in iter(vpi::vpiVariables, h) {
            kids.push(self.walk_var(v.raw(), Some(id))?);
        }
        for m in iter(vpi::vpiMethod, h) {
            kids.push(self.walk_task_func(m.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_gen_scope_array(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::GenScopeArray);
        let mut kids: Vec<NodeId> = Vec::new();
        for gs in iter(vpi::vpiGenScope, h) {
            kids.push(self.walk_gen_scope(gs.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    pub(in crate::core::db) fn walk_gen_scope(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, DbError> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::GenScope);
        let resolved = match self.resolver.scope_params(h) {
            Ok(v) => v.into_iter().collect::<HashMap<String, Val>>(),
            Err(_) => HashMap::new(),
        };
        let mut kids: Vec<NodeId> = Vec::new();
        for p in iter(vpi::vpiParameter, h) {
            let p = p.raw();
            let value = resolved.get(&vpi::obj_name(p)).cloned();
            self.capture_elaborated_type_ranges(h, p);
            kids.push(self.walk_param(p, Some(id), value)?);
        }
        for pa in iter(vpi::vpiParamAssign, h) {
            kids.push(self.walk_param_assign(pa.raw(), Some(id))?);
        }
        // Array handling mirrors `walk_module_inst`: skip the duplicate
        // module-level element nets, walk array_vars/array_nets as arrays.
        let mut array_names: HashSet<String> = HashSet::new();
        for v in iter(vpi::vpiVariables, h) {
            let v = v.raw();
            if is_array_type(vpi::obj_type(v)) {
                let full = vpi::obj_full_name(v);
                if !full.is_empty() {
                    array_names.insert(full);
                }
            }
        }
        for a in iter(vpi::vpiArrayNet, h) {
            let a = a.raw();
            let full = vpi::obj_full_name(a);
            if !full.is_empty() {
                array_names.insert(full);
            }
        }
        for net in iter(vpi::vpiNet, h) {
            let net = net.raw();
            let full = vpi::obj_full_name(net);
            if array_names.contains(&full) {
                continue; // element net of an unpacked array
            }
            self.capture_elaborated_type_ranges(h, net);
            kids.push(self.walk_net(net, Some(id))?);
        }
        for var in iter(vpi::vpiVariables, h) {
            let var = var.raw();
            if is_array_type(vpi::obj_type(var)) {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_array(var, Some(id), false)?);
            } else {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_var(var, Some(id))?);
            }
        }
        for arr in iter(vpi::vpiArrayNet, h) {
            let arr = arr.raw();
            self.capture_elaborated_type_ranges(h, arr);
            kids.push(self.walk_array(arr, Some(id), true)?);
        }
        // Named events elaborated per generate iteration (see
        // `walk_module_inst`); indexed before the scope's processes.
        for ne in iter(vpi::vpiNamedEvent, h) {
            kids.push(self.walk_named_event(ne.raw(), Some(id))?);
        }
        // Instances inside generate scopes (per-iteration interface instances,
        // nested modules) are captured so their signals and ports resolve.
        for iface in iter(vpi::vpiInterface, h) {
            kids.push(self.walk_module_inst(iface.raw(), Some(props.file.as_str()), Some(id))?);
        }
        for c in iter(vpi::vpiModule, h) {
            kids.push(self.walk_module_inst(c.raw(), Some(props.file.as_str()), Some(id))?);
        }
        for ca in iter(vpi::vpiContAssign, h) {
            kids.push(self.walk_cont_assign(ca.raw(), Some(id))?);
        }
        // Structural primitives elaborated per generate iteration (see
        // `walk_module_inst`).
        for p in iter(vpi::vpiPrimitive, h) {
            kids.push(self.walk_primitive(p.raw(), Some(id))?);
        }
        for pa in iter(vpi::vpiPrimitiveArray, h) {
            kids.push(self.walk_primitive_array(pa.raw(), Some(id))?);
        }
        for proc in iter(vpi::vpiProcess, h) {
            kids.push(self.walk_process(proc.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }
}
