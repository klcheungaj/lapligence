// SIM-014 A02: nonblocking event triggers `->>` with delay, event and
// repeated event controls (SV 15.5.1). The issuer does not block; the
// trigger happens in the NBA region when the control completes. The event
// the trigger names is resolved when the statement executes, like the
// target of a nonblocking assignment (SV 10.4.2, 4.9.4), so rebinding or
// nulling the source handle afterwards does not redirect a queued trigger.
// Counts follow the repeat rules of SV 9.4.5 (see intra_counts).
`timescale 1ns / 1ns
module tb;
  event tick;
  event x, y, z, src;
  event evs[0:2];
  int i, n;
  logic [3:0] xn;
  real r;

  // `tick` occurs at 2, 4, 6, ...
  initial forever #2 ->tick;

  always @x $display("%0t x", $time);
  always @y $display("%0t y", $time);
  always @z $display("%0t z", $time);
  always @(evs[0]) $display("%0t evs[0]", $time);
  always @(evs[1]) $display("%0t evs[1]", $time);
  always @(evs[2]) $display("%0t evs[2]", $time);

  task automatic later(event ev, int count);
    ->> repeat (count) @tick ev;
    $display("%0t task issued", $time);
  endtask

  initial begin
    #1 src = x;
    ->> repeat (2) @tick src;
    $display("%0t issued, x.triggered=%0d", $time, x.triggered);
    src = y;
    #2 src = null;
    #4 src = z;
    ->> #3 src;
    src = x;
    #4 src = y;
    ->> @tick src;
    src = null;
    #2 ->> #1 src;
    ->> @tick src;
    #2 i = 1;
    n = 2;
    ->> repeat (n) @tick evs[i];
    i = 2;
    n = 5;
    #4 n = 0;
    ->> repeat (n) @tick evs[0];
    #2 n = -3;
    ->> repeat (n) @tick evs[2];
    #2 xn = 4'bx;
    ->> repeat (xn) @tick evs[1];
    #2 r = 1.5;
    ->> repeat (r) @tick evs[0];
    #6 later(z, 2);
    #6 $finish;
  end
endmodule
