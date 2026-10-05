module tb;
  reg r;
  assign r = 1'b1;
  initial #1 $display("%b", r);
endmodule
