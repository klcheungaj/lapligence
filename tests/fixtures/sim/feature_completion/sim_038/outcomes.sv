// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/outcomes.sv
// SIM-038 A01: failure without an else action, asynchronous disable iff and
// accept_on/reject_on transitions between clock ticks, a same-time-step
// glitch that sampled values do not see, and attempts still pending when the
// simulation ends. Ticks 1..8 are the posedges at 5, 15, ..., 75; stimulus
// changes at 0, 10, ..., 70 and $finish ends the run at 80. The expected
// lines are derived in readme.md.
module tb;
  logic clk = 1'b0;
  logic s1 = 1'b0, b = 1'b0, c = 1'b0;
  logic s2 = 1'b0, e = 1'b0, rst = 1'b0;
  logic s3 = 1'b0, h = 1'b0, g = 1'b0;
  logic s4 = 1'b0, h2 = 1'b0, g2 = 1'b0;
  logic s5 = 1'b0, k = 1'b0, k2 = 1'b1;
  // Bit 7 is tick 1, bit 0 is tick 8.
  logic [7:0] v_s1 = 8'b1000_0000, v_b = 8'b0110_0000;
  logic [7:0] v_s2 = 8'b1001_0000, v_e = 8'b0000_0100;
  logic [7:0] v_s3 = 8'b1000_0000, v_h = 8'b0110_0000;
  logic [7:0] v_s4 = 8'b0000_1000, v_h2 = 8'b0000_0110;
  logic [7:0] v_s5 = 8'b0000_0100;

  n1: assert property (@(posedge clk) s1 |=> (b until c));

  d1: assert property (@(posedge clk) disable iff (rst) s2 |=> s_eventually [0:4] e)
    else $display("%0t d1 fail", $time);
  d1c: cover property (@(posedge clk) disable iff (rst) s2 |=> s_eventually [0:4] e)
    $display("%0t d1 cover", $time);

  ab1: assert property (@(posedge clk) s3 |=> accept_on (g) always [0:3] h)
    $display("%0t ab1 pass", $time); else $display("%0t ab1 fail", $time);
  ab1c: cover property (@(posedge clk) s3 |=> accept_on (g) always [0:3] h)
    $display("%0t ab1 cover", $time);
  ab2: assert property (@(posedge clk) s3 |=> sync_accept_on (g) always [0:3] h)
    else $display("%0t ab2 fail", $time);
  rj1: assert property (@(posedge clk) s3 |=> reject_on (g) always [0:3] h)
    else $display("%0t rj1 fail", $time);
  ab3: assert property (@(posedge clk) s4 |=> accept_on (g2) always [0:2] h2)
    else $display("%0t ab3 fail", $time);

  es: assert property (@(posedge clk) s5 |=> s_eventually k)
    else $display("%0t es fail", $time);
  ew: assert property (@(posedge clk) s5 |=> always k2)
    $display("%0t ew pass", $time); else $display("%0t ew fail", $time);
  sn: assert property (@(posedge clk) s5 |=> s_nexttime [4] k2)
    else $display("%0t sn fail", $time);
  wn: assert property (@(posedge clk) s5 |=> nexttime [4] k)
    else $display("%0t wn fail", $time);
  ens: assert property (@(posedge clk) s5 |=> (k2 s_until k));
  ce: cover property (@(posedge clk) s5 |=> s_eventually k)
    $display("%0t ce cover", $time);

  initial begin
    for (int i = 0; i < 8; i++) begin
      s1 = v_s1[7 - i];
      b = v_b[7 - i];
      s2 = v_s2[7 - i];
      e = v_e[7 - i];
      s3 = v_s3[7 - i];
      h = v_h[7 - i];
      s4 = v_s4[7 - i];
      h2 = v_h2[7 - i];
      s5 = v_s5[7 - i];
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $finish;
  end

  // Asynchronous controls between clock ticks.
  initial begin
    #22 rst = 1'b1;
    #1 rst = 1'b0;
  end
  initial begin
    #27 g = 1'b1;
    #1 g = 1'b0;
  end
  initial begin
    #62 g2 = 1'b1;
    g2 = 1'b0;
  end
endmodule
