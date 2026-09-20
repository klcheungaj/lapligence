// llg-test-fixture: R05 port net-type collapse / arrays
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module plain(inout wire [3:0] p, input wire [3:0] d);
    assign p = d;
endmodule
module and_leaf(inout wand [3:0] p, input wire [3:0] d);
    assign p = d;
endmodule
module up_leaf(inout tri1 p); endmodule
module tb;
    wand [3:0] a [-1:0];
    wire [3:0] b [2:1];
    wire [3:0] selected [0:1];
    reg [3:0] d, other;
    plain u0(a[-1], d);
    and_leaf u1(b[2], d);
    up_leaf u2(selected[0][1]);
    assign a[-1] = other;
    assign b[2] = other;
    initial begin
        d = 4'ha; other = 4'h5;
        #1 $display("arrays=%h%h/%h%h selected=%b/%b", a[-1], a[0], b[2], b[1], selected[0], selected[1]);
        d = 4'hf;
        #1 $display("changed=%h/%h", a[-1], b[2]);
        d = 4'hz; other = 4'hz;
        #1 $display("float=%h/%h", a[-1], b[2]);
        $finish;
    end
endmodule
