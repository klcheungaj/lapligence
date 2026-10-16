// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/concat_targets.v
// IEEE 1364-2001 6.1.3, Table 30; IEEE 1800-2009 10.3.3, 11.4.12. A delayed
// assignment to a concatenation is one vector driver (S2-D6): one delay from
// the whole value, every part together, pending values replaced as a whole,
// and a part's own net delay added after the driver's delay.
`timescale 1ns/1ns
module tb;
    reg [3:0] a;
    wire p, q;
    wire [3:0] v;
    wire #2 n;
    assign #(10,5,3) {p, v[2:1], q} = a;
    assign #4 {n, v[3]} = a[1:0];
    initial begin
        $monitor("%0t p=%b v=%b q=%b n=%b", $time, p, v, q, n);
        a = 4'b0001;
        #20 a = 4'b1000;
        #20 a = 4'b0000;
        #20 a = 4'bzzzz;
        #20 a = 4'b0110;
        #2 a = 4'b0111;
        #20 $finish(0);
    end
endmodule
