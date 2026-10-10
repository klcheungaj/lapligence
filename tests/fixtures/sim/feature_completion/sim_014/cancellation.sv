// SIM-014: cancelling partially completed repeated waits. kill() and
// `disable fork` end a process blocked in `repeat (n) @e` or in a blocking
// `x = repeat (n) @e v` before its count completes (SV 9.6.1, 9.6.3, 9.7);
// the pending blocking assignment never happens, and a new repeated wait
// counts from zero.
`timescale 1ns / 1ns
module tb;
  event e;
  process p;
  int x, y;

  // `e` occurs at 2, 4, 6, ...
  initial forever #2 ->e;

  initial begin
    fork
      begin
        p = process::self();
        repeat (3) @e;
        $display("not reached 1");
      end
    join_none
    #3 p.kill();
    #2 repeat (3) @e;
    $display("%0t new wait counted three events", $time);
    #1 fork
      begin
        p = process::self();
        x = repeat (3) @e 9;
        $display("not reached 2");
      end
    join_none
    #4 p.kill();
    #6 $display("%0t x=%0d", $time, x);
    fork
      begin
        repeat (3) @e;
        $display("not reached 3");
      end
      begin
        y = repeat (3) @e 4;
        $display("not reached 4");
      end
    join_none
    #2 disable fork;
    #2 repeat (2) @e;
    $display("%0t after disable x=%0d y=%0d", $time, x, y);
    $finish;
  end
endmodule
