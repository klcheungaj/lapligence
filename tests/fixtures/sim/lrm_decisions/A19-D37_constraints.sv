// Decision A19-D37: constraints as VPI objects. See docs/lrm_decisions.md
// for the decision and the expected output.
//
// IEEE 1800-2009 37.30 (SystemVerilog-1800-2009.txt L59681-L59723): a
// constraint has vpiVirtual, vpiAutomatic, vpiAllocScheme, vpiName,
// vpiFullName, vpiIsConstraintEnabled, vpiParent (a class obj) and its
// vpiConstraintItem items: constraint orderings, distributions and
// constraint expressions; detail 1 (L59722-L59723): "vpiAutomatic == 0
// implies the constraint was declared static". 37.34 (L59855-L59872): a
// constraint expression is an implication, a constr if, a constr if else or
// an expr. 37.10 detail 5 (L57952-L57954): the "::" separator follows a
// class defn. 37.3.7 (L57548-L57564): vpiDynamicScheme for an object
// allocated in dynamic memory "or part thereof", vpiOtherScheme for all
// other objects. 18.9: constraint_mode of a static constraint affects every
// instance.
//
// Text silent, another simulator followed: a non-static constraint obtained
// from a class defn belongs to no object, and vpiIsConstraintEnabled is
// vpiUndefined (-1) without an error. Text decides: a foreach constraint
// has no object in the 2009 diagrams and is not listed.
module tb;
  class Item;
    rand bit [3:0] a;
    rand bit [3:0] v[2];
    constraint c_inst { a dist { 1 := 3, [4:6] :/ 2, 9 }; { a != 5; } }
    static constraint c_stat { a < 12; }
    constraint c_each { foreach (v[i]) v[i] < 8; a > 0; }
  endclass

  Item it;

  initial begin
    it = new;
    void'(it.randomize());
    it.c_inst.constraint_mode(0);
    it.c_stat.constraint_mode(0);
    $d37();
    $display("done");
  end
endmodule
