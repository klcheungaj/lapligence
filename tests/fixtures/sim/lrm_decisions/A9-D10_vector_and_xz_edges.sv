// Decision A9-D10: z counts as x in event transitions and a change between
// x and z is no transition; a vector terminal makes one event per change,
// an edge event when any bit makes one of the edge's transitions.
//
// IEEE 1800-2009 31.5 (SystemVerilog-1800-2009.txt L52010): "Edge
// transitions involving z are treated the same way as edge transitions
// involving x." 31.8 (L52242-52244): "the transition of one or more bits of
// a vector is considered a single transition of that vector."
`timescale 1ns/1ns
module pe(input d, input c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module ex(input d, input c); reg n; specify $setup(d, edge[0x] c, 10, n); endspecify endmodule
module ho(input d, input c); reg n; specify $hold(posedge c, d, 10, n); endspecify endmodule
module ve(input [3:0] d, input [1:0] c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module tb;
  reg d;
  reg [3:0] pc;                  // 0->z, z->1, x->z, 1->z
  reg ec, hc;
  reg [1:0] hd;                  // hold data: x->z, then z->0
  reg [3:0] vd;
  reg [1:0] v0, v1, v2;          // 00->10, 01->10, 11->10
  pe p0(d, pc[0]); pe p1(d, pc[1]); pe p2(d, pc[2]); pe p3(d, pc[3]);
  ex g(d, ec);
  ho h0(hd[0], hc); ho h1(hd[1], hc);
  ve w0(vd, v0); ve w1(vd, v1); ve w2(vd, v2);
  initial
    $monitor("%0t posedge=%b%b%b%b edge0x=%b hold=%b%b vector=%b%b%b", $time,
             p3.n, p2.n, p1.n, p0.n, g.n, h1.n, h0.n, w2.n, w1.n, w0.n);
  initial begin
    d = 0; pc = 4'b1000; ec = 0; hc = 0; hd = 2'bxx; vd = 0;
    v0 = 2'b00; v1 = 2'b01; v2 = 2'b11;
    #50 pc[1] = 1'bz; pc[2] = 1'bx;
    #45 d = 1; vd = 4'b0110;     // 95: data 5 before
    #5 pc = 4'bzzz1;              // 100: 1->z, x->z, z->z (none), 0->1
    pc[1] = 1;                   // z->1 in the same step
    ec = 1'bz;                   // 0->z is edge[0x]
    v0 = 2'b10; v1 = 2'b10; v2 = 2'b10;
    hc = 1;
    #1 hd = 2'bzz;               // 101: x->z is no data event
    #1 hd[1] = 0;                // 102: z->0 is
    #10 $finish;
  end
endmodule
