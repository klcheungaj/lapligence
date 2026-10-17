// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_instance_arity.v
// IEEE 1364-2001 8.6; IEEE 1800-2009 29.8: terminals follow the definition's port list.
primitive two_inputs (q, a, b);
    output q; reg q;
    input a, b;
    table
        1 0 : ? : 1 ;
    endtable
endprimitive

module tb;
    reg a;
    wire q;
    two_inputs u (q, a);
    initial $finish;
endmodule
