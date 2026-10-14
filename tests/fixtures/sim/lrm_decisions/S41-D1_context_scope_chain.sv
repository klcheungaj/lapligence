// Decision S41-D1: a context import runs in the scope of its declaration;
// svSetScope selects another instance and returns the previous scope; after
// an exported subroutine returns, the chain's scope is again the one it had
// when the export was called. Scope names are full hierarchical names, with a
// loop generate block written `g[0]`.
//
// IEEE 1800-2009 35.5.3 (SystemVerilog-1800-2009.txt L55801-55807):
//   "When a SystemVerilog subroutine calls a DPI context import, a context for
//   the import call chain is created that is equal to the instantiated scope
//   of the import declaration. When a routine in an import call chain invokes
//   svSetScope with a legal argument, the call chain's context is set to the
//   indicated scope. When a call from an import call chain to an exported
//   SystemVerilog subroutine finishes and returns to the chain, the call
//   chain's context is set equal to its value when the call to the export was
//   made."
// H.9.2 (L72598-72599): "The context of an imported or exported subroutine
//   corresponds to the fully qualified name of the subroutine, minus the
//   subroutine name itself."
// H.9.3 (L72702): svGetScopeFromName: "The return value shall be NULL for
//   unrecognized scope names."
// 27.4 (L46316-46319): "the generate block identifier for the loop is
//   indexed by adding the "[genvar value]" to the end of the generate block
//   identifier."
//
// Build S41-D1_context_scope_chain.c into a shared library and load it with
// the simulator's DPI library option.
module leaf_m;
    import "DPI-C" context function int d1_where();
    export "DPI-C" function d1_get;

    int id = 0;

    function int d1_get();
        return id;
    endfunction
endmodule

module tb;
    import "DPI-C" context function int d1_root();
    export "DPI-C" function d1_nested;

    leaf_m u1();
    leaf_m u2();
    for (genvar i = 0; i < 1; i++) begin : g
        leaf_m c();
    end

    // Called from C in scope tb; calls u2's context import.
    function int d1_nested();
        return u2.d1_where();
    endfunction

    int r = 0;

    initial begin
        u1.id = 1;
        u2.id = 2;
        g[0].c.id = 3;
        r = d1_root();
        $display("root=%0d", r);
        $finish;
    end
endmodule
