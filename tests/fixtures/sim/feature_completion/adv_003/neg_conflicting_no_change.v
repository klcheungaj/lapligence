// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_conflicting_no_change.v
// IEEE 1364-2001 8.1.4; IEEE 1800-2009 29.3.4: - keeps state 1, the second row gives 0 for the same inputs.
primitive hold_conflict (q, c, d);
    output q; reg q;
    input c, d;
    table
        1 0 : 1 : - ;
        1 0 : ? : 0 ;
    endtable
endprimitive

module tb;
    reg c, d;
    wire q;
    hold_conflict u (q, c, d);
    initial $finish;
endmodule
