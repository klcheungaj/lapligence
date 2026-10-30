// Gates, a bidirectional switch and their net drivers wait on their inputs
// for the whole run; none of them blocks the end of the run.
module tb;
  wire a = 1'b1;
  wire g, h;
  tran t2 (g, h);
  bufif1 bz (h, a, a);
  initial #1 $display("initial done");
  final $display("final g=%b h=%b", g, h);
endmodule
