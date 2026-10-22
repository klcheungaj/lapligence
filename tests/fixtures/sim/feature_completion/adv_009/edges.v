// Edge-control specifiers, x and z transitions and vector terminals
// (IEEE 1800-2009 31.5, 31.8; IEEE 1364-2001 15.4, 15.7; see readme.md).
`timescale 1ns/1ns
module pe(input d, input c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module ne(input d, input c); reg n; specify $setup(d, negedge c, 10, n); endspecify endmodule
module e01(input d, input c); reg n; specify $setup(d, edge[01] c, 10, n); endspecify endmodule
module ex1(input d, input c); reg n; specify $setup(d, edge[0x, x1] c, 10, n); endspecify endmodule
module ho(input d, input c); reg n; specify $hold(posedge c, d, 10, n); endspecify endmodule
module ve(input [3:0] d, input [1:0] c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module tb;
  reg [5:0] pc;                      // posedge: 0->x, 0->z, x->1, z->1, x->z, 1->x
  reg [1:0] nc;                      // negedge: 1->z, z->x
  reg [2:0] ec;                      // edge[01]: 0->1, 0->x, x->1
  reg [1:0] xc;                      // edge[0x, x1]: 0->1, 0->z
  reg [1:0] hd;                      // hold data: x->z (none), z->1
  reg [1:0] vc0, vc1, vc2;           // vector reference: 00->10, 01->10, 11->10
  reg d;
  reg [3:0] vd;
  pe p0(d, pc[0]); pe p1(d, pc[1]); pe p2(d, pc[2]); pe p3(d, pc[3]); pe p4(d, pc[4]); pe p5(d, pc[5]);
  ne n0(d, nc[0]); ne n1(d, nc[1]);
  e01 a0(d, ec[0]); e01 a1(d, ec[1]); e01 a2(d, ec[2]);
  ex1 b0(d, xc[0]); ex1 b1(d, xc[1]);
  ho h0(hd[0], pc[0]); ho h1(hd[1], pc[0]);
  ve v0(vd, vc0); ve v1(vd, vc1); ve v2(vd, vc2);
  initial
    $monitor("%0t posedge=%b%b%b%b%b%b negedge=%b%b edge01=%b%b%b edge0x_x1=%b%b hold=%b%b vector=%b%b%b",
             $time, p5.n, p4.n, p3.n, p2.n, p1.n, p0.n, n1.n, n0.n, a2.n, a1.n, a0.n,
             b1.n, b0.n, h1.n, h0.n, v2.n, v1.n, v0.n);
  initial begin
    pc = 6'b100000; nc = 2'b01; ec = 0; xc = 0; hd = 2'bxx; d = 0; vd = 0;
    vc0 = 2'b00; vc1 = 2'b01; vc2 = 2'b11;
    #50 pc[2] = 1'bx; pc[3] = 1'bz; pc[4] = 1'bx; nc[1] = 1'bz; ec[2] = 1'bx;
    #45 d = 1; vd[2] = 1;            // 95: data 5 before every reference
    #5 pc[0] = 1'bx; pc[1] = 1'bz; pc[2] = 1; pc[3] = 1; pc[4] = 1'bz; pc[5] = 1'bx;
    nc[0] = 1'bz; nc[1] = 1'bx;
    ec[0] = 1; ec[1] = 1'bx; ec[2] = 1;
    xc[0] = 1; xc[1] = 1'bz;
    vc0 = 2'b10; vc1 = 2'b10; vc2 = 2'b10;
    hd[0] = 1'bz;                    // 100: x->z is no transition
    #1 hd[1] = 1'bz;
    #1 hd[1] = 1;                    // 102: z->1 is a hold data event
    #10 $finish;
  end
endmodule
