// A loop that writes its variable for the last time and ends in one turn,
// while the updates that captured it are pending.
`timescale 1ns/1ns
module tb;
  event ev;
  int v, changes;
  real r;
  initial begin
    fork
      forever @(v) changes++;
    join_none
    for (int i = 0; i < 4; i++) begin
      fork v <= @(ev) i; join_none
      #1;
    end
    for (real x = 0.5; x < 2.0; x += 0.5) begin
      fork r <= @(ev) x; join_none
      #1;
    end
    ->ev;
    #1 $display("%0d %.1f %0d", v, r, changes);
    $finish(0);
  end
endmodule
