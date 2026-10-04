// llg-test-fixture: KI-NET-INTERVAL undriven net-array cells
// IEEE 1800-2009 6.6, 23.3.3.7 and 21.2.1.5: cells without drivers keep their
// net type's undriven value, including the collapsed type of an inout peer.
`timescale 1ns/1ns
module child(inout tri1 [3:0] c [0:7], input wire [3:0] d, input wire en);
    assign c[2] = en ? d : 4'hz;
endmodule
module tb;
    wire [3:0] w [0:7];
    tri0 [1:0] t [0:5];
    supply1 [2:0] s [0:2];
    wire [3:0] free [0:3];
    reg [3:0] d;
    reg en;
    integer i;
    child u(.c(w), .d(d), .en(en));
    assign t[4] = 2'b1z;
    initial begin
        d = 4'h5; en = 1; i = 3;
        #1 $display("w=%h %h %h %h u=%h", w[0], w[2], w[7], w[i], u.c[5]);
        $display("t=%b %b %b", t[0], t[4], t[i]);
        $display("s=%b %b free=%b %b", s[0], s[i - 1], free[0], free[i]);
        $display("v=%v %v %v %v", t[1][0], free[1][2], w[6][1], s[2][0]);
        en = 0;
        #1 $display("w2=%h", w[2]);
        force free[3] = 4'ha;
        #1 $display("forced=%b %b", free[3], free[2]);
        release free[3];
        #1 $display("released=%b", free[3]);
        $finish(0);
    end
endmodule
