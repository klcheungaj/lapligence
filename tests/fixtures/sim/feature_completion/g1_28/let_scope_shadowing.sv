// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_28/let_scope_shadowing.sv
// A let body must resolve free names in its declaration scope even when the
// use site declares a namesake. `bump` reads the module-level `x` (10), not the
// initial-block local `x` (5), so `o` is 11.
module tb;
    logic [7:0] x = 8'd10;
    logic [7:0] o;

    let bump = x + 8'd1;

    initial begin : scope
        logic [7:0] x;
        x = 8'd5;
        o = bump;
        $display("o=%0d", o);
        $finish(0);
    end
endmodule
