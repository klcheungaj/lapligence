// SIM-041 A01/A02: one exported C symbol with several instances selected by
// svSetScope, nested SV -> C -> SV -> C re-entry restoring the caller scope,
// context calls that modify SystemVerilog data, and noncontext/pure imports
// that keep their declared properties inside a context chain.
module unit_m #(parameter int ID = 0);
    int value = 0;

    export "DPI-C" function cell_value;
    export "DPI-C" function cell_bump;
    export "DPI-C" function cell_nest;
    import "DPI-C" context function int cell_visit(input int k);
    // Noncontext: not promoted by the context chain that reaches it.
    import "DPI-C" function int cell_peek();

    function int cell_value(input int k);
        return ID * 100 + value + k;
    endfunction

    // A context call chain modifies this instance's variable. `%m` names the
    // instance the export ran in (a loop generate block reads `g[0]`).
    function void cell_bump(input int d);
        value += d;
        $display("%m(%0d)", d);
    endfunction

    // SV -> C -> SV -> C: an export that re-enters C through a context import
    // of its own instance.
    function int cell_nest(input int k);
        return cell_visit(k) + cell_peek();
    endfunction
endmodule

module tb;
    import "DPI-C" context function int tb_drive();
    import "DPI-C" context function int tb_scopes();
    import "DPI-C" function int plain_twice(input int k);
    import "DPI-C" pure function int pure_add(input int a, input int b);

    unit_m #(1) u1();
    unit_m #(2) u2();
    for (genvar i = 0; i < 2; i++) begin : g
        unit_m #(10 + i) c();
    end

    int sum0;
    int sum1;
    int r;

    initial begin
        r = tb_scopes();
        $display("tb_scopes=%0d", r);
        sum0 = u1.value + u2.value + g[0].c.value + g[1].c.value;
        r = tb_drive();
        sum1 = u1.value + u2.value + g[0].c.value + g[1].c.value;
        $display("tb_drive=%0d", r);
        $display("values %0d %0d %0d %0d (sum %0d -> %0d)", u1.value, u2.value,
                 g[0].c.value, g[1].c.value, sum0, sum1);
        // A context import called directly from SV runs in its declaring scope.
        r = u2.cell_visit(4);
        $display("u2.cell_visit=%0d", r);
        r = g[1].c.cell_nest(5);
        $display("g[1].c.cell_nest=%0d", r);
        $display("pure=%0d plain=%0d", pure_add(2, 3), plain_twice(21));
        $finish;
    end
endmodule
