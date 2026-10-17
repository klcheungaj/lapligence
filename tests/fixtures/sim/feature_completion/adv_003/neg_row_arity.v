// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_row_arity.v
// IEEE 1364-2001 8.1.4; IEEE 1800-2009 29.3.4: one field per input.
primitive short_row (q, a, b);
    output q; reg q;
    input a, b;
    table
        1 : ? : 1 ;
    endtable
endprimitive

module tb;
    reg a, b;
    wire q;
    short_row u (q, a, b);
    initial $finish;
endmodule
