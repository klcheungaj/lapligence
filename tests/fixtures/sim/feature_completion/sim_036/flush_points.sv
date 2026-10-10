// SIM-036 A02: report queues and flush points (IEEE 1800-2009 16.4.1,
// 16.4.2). A process that executes deferred assertions without reaching a
// flush point keeps every report; resuming from an event control or a wait
// statement, or re-triggering an always_comb, clears its queue; a delay
// control (#0) is not a flush point.
module tb;
  logic a = 1'b0, not_a, b = 1'b0;
  logic go = 1'b0, ev = 1'b0;
  int glitches = 0, hits = 0;

  function void glitch();
    glitches++;
    $display("%0d glitch a=%0d not_a=%0d", $time, a, not_a);
  endfunction

  function void hit();
    hits++;
    $display("%0d covered a=%0d b=%0d", $time, a, b);
  endfunction

  task automatic loop_report(input int i);
    $display("%0d loop i=%0d", $time, i);
  endtask

  // The 16.4.2 examples: the transient failure of a2 and the transient
  // coverage of c2 are flushed when the process re-executes.
  assign not_a = !a;
  always_comb begin : comb_check
    a2: assert #0 (not_a != a) else glitch();
  end
  always_comb begin : comb_cover
    c2: cover #0 (b != a) hit();
  end

  initial begin
    // One process, three executions, no flush point: three reports.
    for (int i = 0; i < 3; i++)
      assert #0 (i == 5) else loop_report(i);
    #1;
    // A #0 delay is not a flush point.
    d1: assert #0 (1'b0) else $display("%0d kept across #0", $time);
    #0;
    #1;
    // Resuming from wait() and from @() flushes the pending report.
    w1: assert #0 (1'b0) else $display("%0d BAD wait did not flush", $time);
    wait (go);
    e1: assert #0 (1'b0) else $display("%0d BAD event did not flush", $time);
    @(ev);
    $display("%0d resumed", $time);
    #1;
    // Glitches on combinational inputs.
    a = 1'b1;
    #0 b = 1'b1;
    #1 a = 1'b0;
    #1 $display("%0d glitches=%0d hits=%0d", $time, glitches, hits);
    $finish(0);
  end

  initial begin
    #2;
    #0 go = 1'b1;
    #0 ev = 1'b1;
  end
endmodule
