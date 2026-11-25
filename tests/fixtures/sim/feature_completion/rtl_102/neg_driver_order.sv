// IEEE 1800-2009 6.5: each `a<n>` has two overlapping continuous writes through
// expression ports.
interface bus_if;
  logic [7:0] a0, a1, a2, a3, a4, a5, a6, a7;
  modport m (output .p0(a0[3:0]), output .p1(a1[3:0]), output .p2(a2[3:0]), output .p3(a3[3:0]), output .p4(a4[3:0]), output .p5(a5[3:0]), output .p6(a6[3:0]), output .p7(a7[3:0]));
  modport q (output .r0(a0[5:2]), output .r1(a1[5:2]), output .r2(a2[5:2]), output .r3(a3[5:2]), output .r4(a4[5:2]), output .r5(a5[5:2]), output .r6(a6[5:2]), output .r7(a7[5:2]));
endinterface
module w(bus_if.m x);
  assign x.p0 = 4'd1;
  assign x.p1 = 4'd1;
  assign x.p2 = 4'd1;
  assign x.p3 = 4'd1;
  assign x.p4 = 4'd1;
  assign x.p5 = 4'd1;
  assign x.p6 = 4'd1;
  assign x.p7 = 4'd1;
endmodule
module v(bus_if.q x);
  assign x.r0 = 4'd2;
  assign x.r1 = 4'd2;
  assign x.r2 = 4'd2;
  assign x.r3 = 4'd2;
  assign x.r4 = 4'd2;
  assign x.r5 = 4'd2;
  assign x.r6 = 4'd2;
  assign x.r7 = 4'd2;
endmodule
module tb;
  bus_if bi();
  w u(bi);
  v u2(bi);
  initial $finish(0);
endmodule
