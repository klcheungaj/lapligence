// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_reg_input.v
// IEEE 1364-2001 8.1.2; IEEE 1800-2009 29.3.2: only the output can be declared reg.
primitive reg_input (q, a, b);
    output q; reg q;
    input a, b; reg b;
    table
        1 0 : ? : 1 ;
    endtable
endprimitive

module tb;
    reg a, b;
    wire q;
    reg_input u (q, a, b);
    initial $finish;
endmodule
