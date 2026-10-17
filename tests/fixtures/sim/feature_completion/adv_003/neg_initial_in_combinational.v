// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_initial_in_combinational.v
// IEEE 1364-2001 8.1.2-8.1.3; IEEE 1800-2009 29.3.2-29.3.3: only a sequential UDP has an initial statement.
primitive comb_init (out, a);
    output out;
    input a;
    initial out = 0;
    table
        0 : 1 ;
    endtable
endprimitive

module tb;
    reg a;
    wire out;
    comb_init u (out, a);
    initial $finish;
endmodule
