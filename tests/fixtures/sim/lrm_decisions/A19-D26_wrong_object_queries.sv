// Decision A19-D26: a query the data model does not define for the object.
// vpi_get, vpi_get_str, vpi_handle or vpi_iterate with a property or
// relation that no diagram of Clause 37 gives the reference object's type
// fails: vpi_get returns vpiUndefined, vpi_get_str and vpi_handle return
// NULL, vpi_iterate returns NULL, and each raises a vpiError error that
// vpi_chk_error reports. A relation the diagram does define but that has no
// object (an iteration with no objects, an optional one-to-one target that
// is absent) returns NULL without an error. The answers of every object
// kind are listed in src/sim/rt/vpi_catalog.txt.
//
// IEEE 1800-2009 38.6 (SystemVerilog-1800-2009.txt L62051-L62052): "Should
// an error occur, vpi_get() shall return vpiUndefined."; 38.18 (L63241-
// L63242): "The one-to-one relationships that are traversed with this
// routine are indicated as single arrows in the data model diagrams.";
// 38.23 (L63447-L63453): "vpi_iterate() shall be used to traverse
// one-to-many relationships, which are indicated as double arrows in the
// data model diagrams. ... If there are no objects of type type associated
// with the reference handle ref, then the vpi_iterate() routine shall return
// NULL." The text does not say what a query outside the diagrams returns;
// llg treats it as an error rather than inventing a value. vpiValid is a
// property of a value-change callback's value (deprecated in 38.36.1), not
// of a net.
//
// Build A19-D26_wrong_object_queries.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d26()` declared as a system task).
module tb;
  wire n;
  logic [3:0] mem [0:1];
  int k;
  assign n = 1'b1;
  initial begin : blk
    k = 1;
    assert (k == 1);
    for (int i = 0; i < 2; i++) k += i;
    $d26;
  end
endmodule
