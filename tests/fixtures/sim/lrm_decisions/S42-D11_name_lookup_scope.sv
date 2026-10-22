// Decision S42-D11: vpi_handle_by_name with a NULL scope takes a full
// hierarchical name from a top module; with a scope it resolves the name
// relative to that scope only, so a name of the parent scope is not found.
// Constant selects written in the name (an element, a bit, a part-select)
// return those objects. A name that denotes no object returns NULL with no
// error status (vpi_chk_error returns 0); a select outside the declared range
// of an existing object is an error (S42-D2) (industry-standard behaviour).
//
// IEEE 1800-2009 38.21 (SystemVerilog-1800-2009.txt L63379-63381):
//   "The name can be hierarchical or simple. If scope is NULL, then name
//   shall be searched for from the top level of hierarchy. If a scope object
//   is provided, then search within that scope only."
// Accepting selects inside the name is an llg extension of "all objects with
// a fullname property" (L63379), since element and select handles have
// full names such as tb.mem[1]. Whether a failed lookup is an error is not
// specified. A simple name with a NULL scope is searched from the top level
// only, so "v" is not tb.v.
//
// Build S42-D11_name_lookup_scope.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d11()` declared as a system task).
module child;
    logic [3:0] c = 4'd9;
endmodule
module tb;
    logic [7:0] v = 8'ha5;
    logic [7:0] mem [0:1];
    child u ();
    initial begin
        mem[1] = 8'h3c;
        $d11;
        $finish;
    end
endmodule
