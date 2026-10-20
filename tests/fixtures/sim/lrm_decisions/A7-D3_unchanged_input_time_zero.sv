// Decision A7-D3: an input that has never transitioned counts as having
// transitioned at time 0 when the active paths are chosen. Here B is never
// driven and stays z; Y's first transition, from its initial x (A7-D6) at
// time 0, therefore has both paths active and takes the smaller rise delay,
// B's 5 rather than A's 6.
//
// IEEE 1800-2009 30.5.3 (SystemVerilog-1800-2009.txt L50166-50167): "Active
// specify paths are those whose input has transitioned most recently in
// time". The text does not say how an input that never transitioned
// compares; llg treats it as transitioned at time 0.
`timescale 1ns/1ns
module sel(input A, input B, output Y);
  assign Y = A | B;
  specify
    (A => Y) = (6, 9);
    (B => Y) = (5, 11);
  endspecify
endmodule

module tb;
  reg A;
  wire B;
  wire Y;
  sel s(.A(A), .B(B), .Y(Y));
  initial begin
    $monitor("%0t A=%b B=%b Y=%b", $time, A, B, Y);
    A = 1'b1;
    #100 A = 1'b0;
    #100 $finish;
  end
endmodule
