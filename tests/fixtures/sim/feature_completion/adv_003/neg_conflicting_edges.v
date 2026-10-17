// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_conflicting_edges.v
// IEEE 1364-2001 8.1.4; IEEE 1800-2009 29.3.4: r and p both include (01) with state 0.
primitive edge_conflict (q, c, d);
    output q; reg q;
    input c, d;
    table
        r 0 : ? : 1 ;
        p 0 : 0 : 0 ;
    endtable
endprimitive

module tb;
    reg c, d;
    wire q;
    edge_conflict u (q, c, d);
    initial $finish;
endmodule
