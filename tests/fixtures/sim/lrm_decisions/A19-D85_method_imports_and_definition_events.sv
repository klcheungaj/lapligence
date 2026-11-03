// Decision A19-D85: imports of class methods and the events of a class
// definition's event array property.
// - vpi_iterate(vpiImport, method) of a class method (reached from its
//   class definition through vpiMethods) returns the package items the
//   method's references reached through its own import declarations, as
//   for a task or function of a module (decision A19-D79); a method
//   without such imports gives NULL without an error.
// - vpi_handle_by_index (the declared index) of a named event array
//   property reached from a class definition gives that property's event:
//   a vpiNamedEvent named after its array and index (ea[1]) whose
//   vpiParent is the array. It belongs to no object, so it has no value,
//   no waiting processes and takes no put (errors); vpiNamedEvent of the
//   definition's array stays an error (decision A19-D80). An index outside
//   the range gives NULL and an error.
//
// IEEE 1800-2009 37.12 detail 4 (SystemVerilog-1800-2009.txt L58084-58086):
// "The vpiImport iterator shall return all objects imported into the current
// scope via import declarations. Only objects actually referenced through
// the import shall be returned"; 37.27 detail 2 (L59461-59462):
// "vpi_get_value() and vpi_put_value() are not allowed for variable and
// event handles obtained from class defn handles." The 37.25 diagram gives
// a named event array access by index (vpi_handle_by_index). The full name
// of a method's import follows llg's package-item naming (37.10 detail 5,
// `pk::pv`); some simulators spell it `pk.pv`.
//
// Build A19-D85_method_imports_and_definition_events.c as a VPI application
// (llg: static archive given to --dpi-lib; `task $d85()` declared as a
// system task).
package pk;
  int pv = 3;
endpackage
module tb;
  int g;
  class C;
    event ea[2];
    event eb[3:2];
    task m(input int k);
      import pk::*;
      g = k + pv;
    endtask
    task plain(input int k);
      g = k;
    endtask
  endclass
  initial begin
    C c;
    c = new;
    c.m(5);
    c.plain(1);
    $d85();
    $display("g=%0d", g);
    $finish;
  end
endmodule
