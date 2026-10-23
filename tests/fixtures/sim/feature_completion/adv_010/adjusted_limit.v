// A $hold on the delayed signals of a negative $setuphold (31.9.1): r is
// delayed by 4, so the hold limit 2 is adjusted to 2 + 0 - 4 = -2 and counts
// as 0 (with a warning). The data change at 100, with the undelayed
// reference, is no violation of the delayed signals.
`timescale 1ns/1ns
module nh(input r, input d);
  reg n, nh;
  wire dr, dd;
  integer count = 0;
  time last = 0;
  initial forever @(nh) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge r, d, -3, 8, n, , , dr, dd);
    $hold(posedge r, d, 2, nh);
  endspecify
endmodule
module tb;
  reg r, d;
  nh u(r, d);
  initial begin
    r = 1'b0;
    d = 1'b0;
    #100 r = 1'b1;
    d = 1'b1;
    #50 $display("hold %0d@%0d", u.count, u.last);
    $finish;
  end
endmodule
