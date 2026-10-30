// Decision A19-D29: programs, program arrays, packages and `$unit` as VPI
// instances. vpi_iterate(vpiInstance, NULL) returns `$unit`, the packages
// and the top modules; vpi_iterate(vpiPackage, NULL) the packages and
// `$unit`, whose vpiUnit is TRUE. A package's vpiFullName is its name, an
// object in a package is named `pkg::name` and found by that name or by its
// name below the package, and an object in `$unit` is not found by name. A
// package and a program have no vpiParent (a vpiError); a package's
// vpiInstance is NULL without an error, and a package has no ports. A
// program is a vpiProgram with its instantiating instance as vpiInstance
// and the enclosing module as vpiModule, also for the objects in it. A
// program array is a vpiProgramArray: its elements have it as
// vpiInstanceArray and their index as vpiIndex.
//
// IEEE 1800-2009 37.10 detail 5 (SystemVerilog-1800-2009.txt L57949-
// L57954): "vpiFullName for objects that exist within a compilation unit
// shall begin with '$unit::'. ... vpiFullName for a package shall be the
// name of the package and should end with "::"; ... vpiFullName for
// objects that exist in a package shall begin with the name of the package
// followed by "::"."; detail 6 (L57955-L57957): "The following items
// shall not be accessible via vpi_handle_by_name(): - imported items -
// objects that exist within a compilation unit"; detail 2 (L57934):
// "vpiModule shall return a module if the object is inside a module
// instance, otherwise it shall return NULL."; detail 3 (L57935-L57936):
// "vpiInstance shall always return the immediate instance (package,
// module, interface, or program) in which the object is instantiated.";
// 37.9 detail 1 (L57869-L57870): "If a program is an element within an
// instance array, the vpiIndex transition is used to access the index
// within the array. If a program is not part of an instance array, this
// transition shall return NULL."; the 37.10 diagram (L57882-L57932) makes
// the instance class (package, interface, program, module) reachable from
// NULL and lists program and program array among the instance items;
// 37.11 (L57974-L57990) lists the program array among the instance
// arrays. The text gives instances no vpiParent and a package no vpiInstance
// or ports, and only recommends ("should") the `::` at the end of a
// package's own full name; there llg follows industry-standard behaviour:
// the package's full name is its plain name, vpiParent of a package or
// program is a vpiError, vpiInstance of a package is NULL without one.
package pkg;
  int pv = 3;
  function automatic int pf(int a);
    return a + 1;
  endfunction
endpackage

int uv = 9;

program prg (input logic clk);
  int x = 1;
  initial #10 x = pkg::pf(pkg::pv) + uv;
endprogram

module tb;
  logic clk = 0;
  prg p (clk);
  prg pa [1:0] (clk);
  initial begin
    $d29;
    $finish;
  end
endmodule
