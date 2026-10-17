`timescale 1ns/1ns
// ADV-007 A01: a path destination is inertial with the default pulse limits
// (reject and error limits equal to the delay): an input pulse shorter than
// the path delay never reaches the output; a longer one is delayed whole.
module pcell(input a, output y);
  assign y = a;
  specify
    (a => y) = (3, 8);
  endspecify
endmodule

module tb;
  reg a;
  wire y;
  pcell u(.a(a), .y(y));
  initial $monitor("%0t a=%b y=%b", $time, a, y);
  initial begin
    a = 1'b0;
    #100 a = 1'b1;
    #5 a = 1'b0;
    #95 a = 1'b1;
    #2 a = 1'b0;
    #98 a = 1'b1;
    #50 a = 1'b0;
    #50 $finish;
  end
endmodule
