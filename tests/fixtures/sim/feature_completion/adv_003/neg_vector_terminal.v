// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_vector_terminal.v
// IEEE 1364-2001 8.1.1, 8.6; IEEE 1800-2009 29.3.1, 29.8: a single instance takes scalar terminals.
primitive scalar_dff (q, clk, d);
    output q; reg q;
    input clk, d;
    table
        r 0 : ? : 0 ;
        r 1 : ? : 1 ;
    endtable
endprimitive

module tb;
    reg clk;
    reg [1:0] d;
    wire q;
    scalar_dff u (q, clk, d);
    initial $finish;
endmodule
