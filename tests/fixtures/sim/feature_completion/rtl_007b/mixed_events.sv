// Event lists that combine process-evaluated helpers with named events, and
// real-valued helpers (SV 9.4.2, 9.4.2.3, 15.5).
`timescale 1ns / 1ns
module tb;
  event ev, ev2;
  int a = 0, seen = 0, gate = 0;
  real rv = 0.0;
  int w1 = -1, w2 = -1, w3 = -1, w4 = -1, w5 = -1, w6 = -1;

  function int f(input int v);
    seen++;
    return v;
  endfunction

  function real g(input real v);
    seen++;
    return v * 2.0;
  endfunction

  function int ok(input int v);
    seen++;
    return v;
  endfunction

  // A named event wakes the list.
  initial begin
    @(ev or f(a));
    w1 = $time;
  end

  // A helper change wakes the list.
  initial begin
    #2 @(ev2 or f(a));
    w2 = $time;
  end

  // A trigger before the control is reached is not an event (SV 15.5.2).
  initial begin
    #5 -> ev2;
    @(ev2 or f(a));
    w3 = $time;
  end

  // A qualifier with an effectful helper filters a named event.
  initial begin
    @(ev iff ok(gate));
    w4 = $time;
  end

  // Real any-change: an unchanged real value is no event.
  initial begin
    @(g(rv));
    w5 = $time;
  end

  // Mixed real and named-event list.
  initial begin
    #11 @(g(rv) or ev2);
    w6 = $time;
  end

  initial begin
    #1 -> ev;
    #2 a = 1;
    #3 a = 2;
    #1 rv = 0.0;
    #1 gate = 1;
    #1 -> ev;
    #1 rv = 1.25;
    #2 -> ev2;
    #1 $display("w1=%0d w2=%0d w3=%0d w4=%0d w5=%0d w6=%0d seen_ok=%0d",
                w1, w2, w3, w4, w5, w6, seen >= 6);
    $finish(0);
  end
endmodule
