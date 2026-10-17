// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_missing_state.v
// IEEE 1364-2001 8.1.4; IEEE 1800-2009 29.3.4: a sequential row has a current-state field.
primitive seq_nostate (q, a);
    output q; reg q;
    input a;
    table
        1 : 1 ;
    endtable
endprimitive

module tb;
    reg a;
    wire q;
    seq_nostate u (q, a);
    initial $finish;
endmodule
