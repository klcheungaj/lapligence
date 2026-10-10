// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/disable.sv
// SIM-038 A01: disable conditions. `disable iff` takes any Boolean
// expression of current values (16.12), a `default disable iff` covers the
// concurrent assertions of its module and generate blocks unless they name
// their own condition or the block declares another default (16.16), and it
// reaches neither child instances nor procedural expect statements. Every
// attempt below starts on tick 1 (posedge at 5) and fails on tick 5 (45)
// unless a disable pulse between ticks removes it. The expected lines are
// derived in readme.md.
module child(input logic clk, s, t);
  c1: assert property (@(posedge clk) s |=> s_eventually [0:3] t)
    else $display("%0t c1 fail", $time);
endmodule

module tb;
  logic clk = 1'b0, s = 1'b0, t = 1'b0;
  logic rst_n = 1'b1, c = 1'b0, d = 1'b0, kill_m = 1'b0, kill_g = 1'b0;

  default disable iff (kill_m);

  x0: assert property (@(posedge clk) disable iff (1'b0) s |=> s_eventually [0:3] t)
    else $display("%0t x0 fail", $time);
  x1: assert property (@(posedge clk) disable iff (!rst_n) s |=> s_eventually [0:3] t)
    else $display("%0t x1 fail", $time);
  x2: assert property (@(posedge clk) disable iff (c || d) s |=> s_eventually [0:3] t)
    else $display("%0t x2 fail", $time);
  y1: assert property (@(posedge clk) s |=> s_eventually [0:3] t)
    else $display("%0t y1 fail", $time);

  if (1) begin : g
    default disable iff (kill_g);
    y2: assert property (@(posedge clk) s |=> s_eventually [0:3] t)
      else $display("%0t y2 fail", $time);
  end

  child u_child(.clk(clk), .s(s), .t(t));

  initial begin
    #2;
    expect (@(posedge clk) s ##1 !t ##1 !t ##1 !t)
      $display("%0t e pass", $time);
    else $display("%0t e fail", $time);
  end

  initial begin
    s = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    s = 1'b0;
    repeat (6) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $finish;
  end

  // Disable pulses between clock ticks.
  initial begin
    #22 rst_n = 1'b0;
    #1 rst_n = 1'b1;
  end
  initial begin
    #27 kill_m = 1'b1;
    #1 kill_m = 1'b0;
  end
  initial begin
    #32 d = 1'b1;
    #1 d = 1'b0;
  end
endmodule
