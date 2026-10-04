// IEEE 1800-2009 25.5: an input modport port cannot be assigned.
interface bus_if;
  logic [7:0] a;
  modport m (input .p(a[3:0]));
endinterface
module w(bus_if.m x);
  initial x.p = 4'd1;
endmodule
module tb;
  bus_if bi();
  w u(bi);
  initial $finish(0);
endmodule
