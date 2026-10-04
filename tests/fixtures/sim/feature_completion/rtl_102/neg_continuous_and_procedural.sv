// IEEE 1800-2009 6.5: a continuous write through .p(a[3:0]) and a procedural write to a[0] overlap.
interface bus_if;
  logic [7:0] a;
  modport m (output .p(a[3:0]));
endinterface
module w(bus_if.m x);
  assign x.p = 4'd1;
endmodule
module tb;
  bus_if bi();
  w u(bi);
  initial begin bi.a[0] = 1'b0; $finish(0); end
endmodule
