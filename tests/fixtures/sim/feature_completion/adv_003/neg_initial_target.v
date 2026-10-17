// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_initial_target.v
// IEEE 1364-2001 8.5 Table 41; IEEE 1800-2009 29.7: the initial statement assigns the output.
primitive init_input (q, a);
    output q; reg q;
    input a;
    initial a = 1;
    table
        1 : ? : 1 ;
    endtable
endprimitive

module tb;
    reg a;
    wire q;
    init_input u (q, a);
    initial $finish;
endmodule
