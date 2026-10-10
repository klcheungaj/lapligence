// SIM-036 A01: one function holding a deferred assertion, called by two
// always_comb processes (IEEE 1800-2009 16.4.5). Each process owns its own
// report queue; its re-triggering flushes only its own reports, and every
// report prints the issue-time values of the function's automatic formals.
module tb;
  logic [3:0] x, y, z, w;
  logic s1, s2;

  function automatic void report(input string who, input logic [3:0] a,
                                 input logic [3:0] b);
    $display("%0d %s a=%0d b=%0d", $time, who, a, b);
  endfunction

  function automatic logic f(input logic [3:0] a, input logic [3:0] b,
                             input string who);
    a1: assert #0 (a == b) else report(who, a, b);
    return a == b;
  endfunction

  always_comb begin : b1
    s1 = f(x, y, "b1");
  end

  always_comb begin : b2
    s2 = f(z, w, "b2");
  end

  initial begin
    // Time step 1: both processes fail independently.
    x = 1; y = 2; z = 3; w = 4;
    #1;
    // Time step 2: b1 fails, then passes when y settles (flushed); b2 passes.
    x = 5; y = 6; z = 7; w = 7;
    #0 y = 5;
    #1;
    // Time step 3: b1 fails with no flush point; b2 passes.
    x = 1; z = 2; w = 2;
    #1;
    $finish(0);
  end
endmodule
