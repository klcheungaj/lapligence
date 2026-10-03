// RTL-011: one alias network seen through a packed-structure member, a
// vector part select and an inout port shares writes, force/release and
// waveform values (IEEE 1800-2009 10.11, 10.6.2, 21.7).
`timescale 1ns/1ns
typedef struct packed { logic [1:0] hi; logic [1:0] lo; } pair_t;
module child(inout wire [1:0] p, input [1:0] d); assign p = d; endmodule
module tb;
    wire pair_t s;
    wire [3:0] v;
    wire [1:0] m;
    reg [1:0] cd, hd;
    alias s.lo = m;
    alias v[3:2] = s.hi;
    child c(m, cd);
    assign v[1:0] = 2'b01;
    assign s.hi = hd;
    initial begin
        $monitor("%0t %b %b %b %b", $time, s, v, m, c.p);
        $dumpfile("alias_identity.vcd");
        $dumpvars(0, tb);
        cd = 2'b10; hd = 2'b11;
        #1 force m[0] = 1'b1;
        #1 force v[3] = 1'b0;
        #1 cd = 2'b00; hd = 2'b10;
        #1 release m[0];
        #1 release v[3];
        #1 $finish(0);
    end
endmodule
