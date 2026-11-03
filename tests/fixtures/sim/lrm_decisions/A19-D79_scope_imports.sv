// Decision A19-D79: vpi_iterate(vpiImport, scope) returns the package items
// that name references reached through the import declarations of that
// scope: an explicit import that some reference used, every name a wildcard
// import supplied, and, in a package that re-exports an item (26.6), the
// item when a reference through the exporting package reached it. A
// reference in a nested scope counts for the scope whose import it went
// through. Only instance items (37.10) are listed: variables, named events,
// parameters, tasks, functions and class definitions; a typedef or an enum
// constant is not one. A scope whose imports no reference used, a scope
// without imports and a package give NULL without an error. There is no
// one-to-one vpiImport relation: vpi_handle(vpiImport, scope) is an error.
// The order of the items is not specified; the companion sorts them.
//
// IEEE 1800-2009 37.12 detail 4 (SystemVerilog-1800-2009.txt L58084-58086):
// "The vpiImport iterator shall return all objects imported into the current
// scope via import declarations. Only objects actually referenced through
// the import shall be returned, rather than items potentially made visible
// as a result of the import. Refer to 26.3 for more details." The 37.12
// diagram (L58059-58060) names the target "instance item"; the 37.10
// diagram lists program, program array, task func, net, array net,
// variables, named event, named event array, parameters, spec param,
// assertion and class defn as instance items. Industry-standard behaviour,
// except that some simulators list only the variables, named events and
// parameters an import supplied, omitting the referenced task, function
// and class definition items, and list an explicitly imported variable or
// parameter that no reference uses (here `unused_v` of tb and `qw` of
// tb.blk); llg follows detail 4 ("Only objects actually referenced through
// the import") and the diagram.
//
// Build A19-D79_scope_imports.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d79()` declared as a system task).
package pk;
    int pv = 3;
    int unused_v = 0;
    parameter int PP = 5;
    typedef logic [3:0] nib_t;
    function int pf(int a);
        return a + 1;
    endfunction
    int px = 9;
endpackage

package pk2;
    int qv = 4;
    int qw = 8;
endpackage

package pk3;
    import pk::px;
    export pk::px;
    int rv = 1;
endpackage

module sub;
    import pk::pv;
    initial #1 $display("sub pv=%0d", pv);
endmodule

module tb;
    import pk::pv;
    import pk::PP;
    import pk::nib_t;
    import pk::pf;
    import pk::unused_v;
    import pk2::*;
    nib_t n = 4'h3;
    sub u ();
    function int f2;
        import pk3::*;
        return rv + px;
    endfunction
    initial begin : blk
        import pk2::qw;
        #2 $display("tb %0d %0d %0d %0d %0d", pv, pv, PP, pf(qv), n);
        $display("f2=%0d", f2());
        $d79;
        $finish;
    end
endmodule
