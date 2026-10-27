// llg-test-fixture: ADV-017 static assertion information (IEEE 1800-2009
// 39.3) and callbacks that control their own assertion (39.4.2, 39.5.2).
// objects.c ($objects, $summary) walks the assertions of the design, its
// instances and a checker instance, and kills an attempt from inside its
// own start callback. Expected output: objects.out (derivation in
// readme.md).
`timescale 1ns/1ns
checker chk(logic x, event ck);
    k_x: assert property (@ck x);
endchecker

module leaf(input logic clk, input logic v);
    l_v: assume property (@(posedge clk) v);
    always @(posedge clk) begin
        assert (v);
    end
endmodule

module tb;
    logic clk = 0, v = 1;

    always #5 clk = ~clk;

    r1: assert property (@(posedge clk) disable iff (!v) v |=> v);
    c1: cover sequence (@(posedge clk) v ##1 v);
    always @(posedge clk) begin
        d1: assert #0 (v);
        i1: cover (v);
    end
    leaf u_leaf(clk, v);
    chk u_chk(v, posedge clk);

    initial begin
        $objects;
        #40 $summary;
        $finish;
    end
endmodule
