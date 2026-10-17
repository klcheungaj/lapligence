// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_no_change_in_combinational.v
// IEEE 1364-2001 8.1.6 Table 40; IEEE 1800-2009 29.3.6: - is only an output of a sequential UDP.
primitive comb_hold (out, a);
    output out;
    input a;
    table
        1 : - ;
    endtable
endprimitive

module tb;
    reg a;
    wire out;
    comb_hold u (out, a);
    initial $finish;
endmodule
