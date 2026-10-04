// IEEE 1800-2009 25.5.4: selectors in a port expression are constant expressions.
interface bus_if;
  logic [7:0] a;
  logic [2:0] idx;
  modport m (output .p(a[idx]));
endinterface
module w(bus_if.m x);
  assign x.p = 1'b1;
endmodule
module tb;
  bus_if bi();
  w u(bi);
  initial begin bi.idx = 2; #1 $display("%b", bi.a); $finish(0); end
endmodule
