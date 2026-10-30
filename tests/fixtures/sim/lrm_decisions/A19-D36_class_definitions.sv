// Decision A19-D36: class definitions, methods and the randomization,
// visibility and const properties of variables as VPI objects. See
// docs/lrm_decisions.md for the decision and the expected output.
//
// IEEE 1800-2009 37.10 detail 5 (SystemVerilog-1800-2009.txt L57952-L57954):
// "vpiFullName for objects that exist in a package shall begin with the name
// of the package followed by "::". ... The . separator shall be used in all
// cases except package and class defn." 37.10 detail 2 (L57934): "vpiModule
// shall return a module if the object is inside a module instance,
// otherwise it shall return NULL." 37.37 detail 5 (L60004-L60005):
// "vpiFullName of a task or function declared inside a package or class
// defn shall begin with the full name of the package or class defn followed
// by "::" and immediately followed with the name of the task or function."
// 37.27 detail 1 (L59458-L59460): "the iteration over vpiMethods shall not
// include built-in methods for which there is no explicit declaration."
// 37.27 diagram (L59513-L59514): a class defn iterates its vpiInternalScope
// scopes. 37.27 detail 6 (L59478-L59479): "The relation from extends to
// class typespec provides the base class." 37.17 details 15, 16 and 24
// (L58775-L58777, L58822-L58824): vpiRandType is the current randomization
// type of the variable; vpiIsRandomized tells whether a random variable is
// currently active for randomization; vpiVisibility is vpiPublicVis "for a
// variable that is not a class member".
//
// Text silent, reference followed: vpi_iterate(vpiClassDefn, NULL) lists the
// class definitions of the whole design, and a property obtained from a
// class defn is not randomized (it belongs to no object).
package d36_pkg;
  class PkgClass;
    function int get();
      return 1;
    endfunction
  endclass
endpackage

class UnitClass;
  int u;
endclass

module tb;
  class Base;
    rand bit [3:0] a;
    rand int b;
    local int hidden = 2;
    function int f(input int k, ref int r);
      return k + r + hidden;
    endfunction
    function real g();
      return 1.5;
    endfunction
    function bit [3:0] h();
      return a;
    endfunction
  endclass

  class Child extends Base;
    task t();
    endtask
  endclass

  Child obj;
  const int limit = 3;
  int plain;
  d36_pkg::PkgClass pc;
  UnitClass uc;

  initial begin
    obj = new;
    obj.b.rand_mode(0);
    void'(std::randomize(plain) with { plain < 10; });
    pc = new;
    uc = new;
    $d36();
    $display("done %0d", limit + pc.get() + uc.u);
  end
endmodule
