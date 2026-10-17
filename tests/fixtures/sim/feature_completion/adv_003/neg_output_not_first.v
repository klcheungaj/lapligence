// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/neg_output_not_first.v
// IEEE 1364-2001 8.1.1; IEEE 1800-2009 29.3.1: the output port is the first port.
primitive input_first (a, q);
    input a;
    output q; reg q;
    table
        1 : ? : 1 ;
    endtable
endprimitive

module tb;
    reg a;
    wire q;
    input_first u (a, q);
    initial $finish;
endmodule
