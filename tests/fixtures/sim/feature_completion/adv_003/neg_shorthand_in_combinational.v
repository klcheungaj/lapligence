// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_shorthand_in_combinational.v
// IEEE 1364-2001 8.1.6 Table 40; IEEE 1800-2009 29.3.6: r f p n * need a sequential UDP.
primitive comb_rise (out, clk, data);
    output out;
    input clk, data;
    table
        r 0 : 1 ;
    endtable
endprimitive

module tb;
    reg clk, data;
    wire out;
    comb_rise u (out, clk, data);
    initial $finish;
endmodule
