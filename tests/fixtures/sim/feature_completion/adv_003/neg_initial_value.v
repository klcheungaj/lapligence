// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_initial_value.v
// IEEE 1364-2001 8.1.3, 8.5 Table 41; IEEE 1800-2009 29.3.3, 29.7: a single-bit 0, 1 or x literal.
primitive wide_init (q, a);
    output q; reg q;
    input a;
    initial q = 2'b10;
    table
        1 : ? : 1 ;
    endtable
endprimitive

module tb;
    reg a;
    wire q;
    wide_init u (q, a);
    initial $finish;
endmodule
