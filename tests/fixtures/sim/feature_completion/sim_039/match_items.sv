// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/match_items.sv
`timescale 1ns / 1ns
// SIM-039 A01: repeated and ordered match items (IEEE 1800-2009 16.10,
// 16.11). Cycle n is the posedge at 10n+5. a = 1 in cycles 1-3, b = 1 in
// cycles 2-4, d = 1, 2, 4, 8 in cycles 1-4, and q counts posedges with a
// nonblocking update. Every iteration of a repeated body runs its items on
// its own attempt's locals; a call's by-value arguments are the sampled
// values and locals at its place in the list, while its body runs later in
// the Reactive region and reads current values. Derivation: readme.md.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  logic [7:0] d = 8'd0, q = 8'd0;
  int calls = 0;

  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 8'd1;

  function automatic void note(input int k, input logic [7:0] x);
    calls++;
    $display("%0t note %0d x=%0d", $time, k, x);
  endfunction

  task automatic tnote(input logic [7:0] x);
    $display("%0t tnote x=%0d", $time, x);
  endtask

  function automatic void seen(input logic [7:0] arg);
    $display("%0t seen arg=%0d now=%0d", $time, arg, q);
  endfunction

  sequence rep;
    logic [7:0] x = 8'd0;
    (a, x += d, note(1, x)) [*3] ##1 (b, note(2, x), x = x + 8'd1, tnote(x));
  endsequence

  sequence cnt;
    int n;
    ($rose(a), n = 1) ##1 (a, n++) [*0:$] ##1 (!a, $display("%0t cnt n=%0d", $time, n));
  endsequence

  c_rep: cover sequence (@(posedge clk) rep);
  c_cnt: cover sequence (@(posedge clk) cnt);
  c_seen: cover sequence (@(posedge clk) (a, seen(q)));

  initial begin
    @(negedge clk) a = 1'b1; d = 8'd1;     // cycle 1
    @(negedge clk) b = 1'b1; d = 8'd2;     // cycle 2
    @(negedge clk) d = 8'd4;               // cycle 3
    @(negedge clk) a = 1'b0; d = 8'd8;     // cycle 4
    @(negedge clk) b = 1'b0;               // cycle 5
    @(negedge clk) $display("%0t calls=%0d", $time, calls);
    $finish;
  end
endmodule
