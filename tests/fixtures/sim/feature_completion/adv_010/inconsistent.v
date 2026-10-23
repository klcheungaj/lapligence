// Mutually inconsistent negative limits (31.9.1): each check needs its
// reference delayed more than its data, and the data of one is the reference
// of the other. The smallest negative limit (-6) and then -5 count as 0, so
// no signal is delayed and both checks keep hold windows of 10 after their
// reference edge.
`timescale 1ns/1ns
module cyc(input A, input B);
  reg n;
  wire dA, dB;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge A, B, -5, 10, n, , , dA, dB);
    $setuphold(posedge B, A, -6, 10, n, , , dB, dA);
  endspecify
endmodule
module tb;
  reg a, b;
  cyc u(a, b);
  always @(u.dA) if ($time > 50) $display("%0t dA=%b", $time, u.dA);
  always @(u.dB) if ($time > 50) $display("%0t dB=%b", $time, u.dB);
  initial begin
    a = 1'b0;
    b = 1'b0;
    #100 a = 1'b1;
    #6 b = 1'b1;
    #100 $display("%0d@%0d", u.count, u.last);
    $finish;
  end
endmodule
