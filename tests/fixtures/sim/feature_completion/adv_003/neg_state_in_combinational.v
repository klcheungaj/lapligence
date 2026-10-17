// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_state_in_combinational.v
// IEEE 1364-2001 8.1.4; IEEE 1800-2009 29.3.4: only sequential rows have a current-state field.
primitive comb_state (out, a);
    output out;
    input a;
    table
        1 : 0 : 1 ;
    endtable
endprimitive

module tb;
    reg a;
    wire out;
    comb_state u (out, a);
    initial $finish;
endmodule
