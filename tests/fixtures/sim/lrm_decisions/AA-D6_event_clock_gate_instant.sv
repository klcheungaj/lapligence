// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D6_event_clock_gate_instant.sv
// Decision AA-D6: the `$past` gating expression of an event-list or
// named-event clock is evaluated when the event occurs, like an `iff`
// qualifier, not later in the time step.
//
// IEEE 1800-2009 16.9.3 (SystemVerilog-1800-2009.txt L22767-22770): "the
//   particular time step is the kth strictly prior time step in which the
//   event ev iff expression2 occurred. If there do not exist k strictly prior
//   time steps in which the event ev iff expression2 occurred, then the value
//   returned from the $past function is the result of evaluating expression1
//   using the initial values of the variables comprising it."
// 9.4.2.3 (L11987-11988): "The event expression only triggers if the
//   expression after the iff is true [...] This type of expression is
//   evaluated when a changes and not when enable changes."
//
// In each case one statement sequence raises the event while `g` is 1 and
// then clears `g` in the same time step, so the event occurred with
// `ev iff g` true and is a clock tick.
// - Event list `@(posedge a or posedge b)`: tick at time 2, where the sampled
//   `v` is 2; at time 4 `$past` returns 2.
// - Named event `@(ev)`: tick at time 6, where the sampled `v` is 5; at time
//   8 `$past` returns 5.
// Had the gate been read after the event, neither would tick and both calls
// would return the initial value 1.
module tb;
  logic a = 1'b0, b = 1'b0, g = 1'b1;
  logic [3:0] v = 4'd1;
  event ev;

  initial begin
    #1 v = 4'd2;
    #1 begin
      a = 1'b1;
      g = 1'b0;
    end
    #1 v = 4'd3;
    #1 $display("event list %0d", $past(v, 1, g, @(posedge a or posedge b)));
    #1 begin
      v = 4'd5;
      g = 1'b1;
    end
    #1 begin
      ->ev;
      g = 1'b0;
    end
    #1 v = 4'd7;
    #1 $display("named event %0d", $past(v, 1, g, @(ev)));
    $finish;
  end
endmodule
