// $info is no warning: `-Werror` leaves it alone (IEEE 1800-2009 20.10).
`timescale 1ns/1ps
module tb;
  $info("hi");
  initial $finish;
endmodule
