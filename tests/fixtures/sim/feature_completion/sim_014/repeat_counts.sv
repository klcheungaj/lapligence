// SIM-014: standalone repeated event waits `repeat (n) @e;` (V2001 9.6,
// SV 12.7.2, 9.4.5). The count is evaluated once when the loop starts; X/Z
// counts run zero times, nonpositive signed counts run zero times, an
// unsigned count keeps its unsigned value, and a real count converts to an
// integer by rounding, ties away from zero (SV 6.12.2).
`timescale 1ns / 1ns
module tb;
  event e;
  int n;
  logic [1:0] u2;
  logic signed [3:0] s4;
  logic [7:0] xv;
  real r;
  shortreal sr;

  // `e` occurs at 2, 4, 6, ...; every wait below starts at an odd time.
  initial forever #2 ->e;

  initial begin
    #1;
    n = 3;
    repeat (n) @e;
    $display("%0t int 3", $time);
    #1 n = 0;
    repeat (n) @e;
    $display("%0t int 0", $time);
    n = -2;
    repeat (n) @e;
    $display("%0t int -2", $time);
    s4 = -1;
    repeat (s4) @e;
    $display("%0t signed4 -1", $time);
    u2 = 2'b11;
    repeat (u2) @e;
    $display("%0t unsigned2 3", $time);
    #1 xv = 8'bx;
    repeat (xv) @e;
    $display("%0t x", $time);
    xv = 8'b0000_001z;
    repeat (xv) @e;
    $display("%0t z", $time);
    r = 1.4;
    repeat (r) @e;
    $display("%0t real 1.4", $time);
    #1 r = 1.5;
    repeat (r) @e;
    $display("%0t real 1.5", $time);
    #1 r = -0.4;
    repeat (r) @e;
    $display("%0t real -0.4", $time);
    sr = 2.0;
    repeat (sr) @e;
    $display("%0t shortreal 2.0", $time);
    #1 repeat (0.5) @e;
    $display("%0t literal 0.5", $time);
    #1 n = 2;
    fork
      begin
        repeat (n) @e;
        $display("%0t count fixed at start", $time);
      end
      #2 n = 10;
    join
    $finish;
  end
endmodule
