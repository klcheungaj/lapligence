// IEEE 1800-2009 6.5: bit 1 of .p({a[1:0], c}) is a[0], also continuously driven.
interface bus_if;
  logic [7:0] a;
  logic c;
  modport m (output .p({a[1:0], c}));
endinterface
module w(bus_if.m x, input logic go);
  always_ff @(posedge go) x.p[1] <= 1'b1;
endmodule
module tb;
  bus_if bi(); logic go = 0;
  w u(bi, go);
  assign bi.a[0] = 1'b0;
  initial $finish(0);
endmodule
