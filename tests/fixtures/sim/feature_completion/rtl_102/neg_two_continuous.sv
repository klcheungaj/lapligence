// IEEE 1800-2009 6.5: two continuous writes through .p(a[3:0]) and .r(a[5:2]) overlap.
interface bus_if;
  logic [7:0] a;
  modport m (output .p(a[3:0]));
  modport q (output .r(a[5:2]));
endinterface
module w(bus_if.m x);
  assign x.p = 4'd1;
endmodule
module v(bus_if.q x);
  assign x.r = 4'd2;
endmodule
module tb;
  bus_if bi();
  w u(bi);
  v u2(bi);
  initial $finish(0);
endmodule
