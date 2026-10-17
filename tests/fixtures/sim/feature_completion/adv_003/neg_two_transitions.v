// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_two_transitions.v
// IEEE 1364-2001 8.1.4, 8.4; IEEE 1800-2009 29.3.4, 29.6: at most one input transition per row.
primitive two_edges (q, a, b);
    output q; reg q;
    input a, b;
    table
        (01) (10) : ? : 1 ;
    endtable
endprimitive

module tb;
    reg a, b;
    wire q;
    two_edges u (q, a, b);
    initial $finish;
endmodule
