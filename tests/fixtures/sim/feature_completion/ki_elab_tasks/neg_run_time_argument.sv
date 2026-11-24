// "list_of_arguments may only contain a formatting string and constant
// expressions" (IEEE 1800-2009 20.10).
`timescale 1ns/1ps
module tb;
  int v = 3;
  $info("%0d", v);
  initial $display("must not run %0d", v);
endmodule
