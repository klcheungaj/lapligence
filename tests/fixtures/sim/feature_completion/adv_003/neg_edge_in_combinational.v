// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_edge_in_combinational.v
// IEEE 1364-2001 8.1.6 Table 40; IEEE 1800-2009 29.3.6: (vw) needs a sequential UDP.
primitive comb_edge (out, data);
    output out;
    input data;
    table
        (01) : 1 ;
    endtable
endprimitive

module tb;
    reg data;
    wire out;
    comb_edge u (out, data);
    initial $finish;
endmodule
