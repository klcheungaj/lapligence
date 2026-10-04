// IEEE 1800-2009 25.5.4: an output port expression must be a valid lvalue.
interface bus_if;
  logic [7:0] a, b;
  modport m (output .p(a + b));
endinterface
module w(bus_if.m x);
  assign x.p = 8'd1;
endmodule
module tb;
  bus_if bi();
  w u(bi);
  initial $finish(0);
endmodule
