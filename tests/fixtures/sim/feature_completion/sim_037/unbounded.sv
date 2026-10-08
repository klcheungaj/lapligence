// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/unbounded.sv
// SIM-037 A03: unbounded obligations stay pending until they are met or can
// no longer be met, and never complete at a finite cutoff (IEEE 1800-2009
// 16.9.2, 16.9.5-16.9.6, 16.12). Passes come from the cover twins, which
// ignore vacuous successes; failures come from the assert else actions.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic a = 1'b0;
  logic b = 1'b0;
  logic c = 1'b0;
  int t = 0;

  u1: cover property (@(posedge clk) go |-> ##[1:$] b) $display("U1 pass t=%0d", t);
  u1f: assert property (@(posedge clk) go |-> ##[1:$] b) else $display("U1 fail t=%0d", t);
  u2: cover property (@(posedge clk) go |-> b[->2]) $display("U2 pass t=%0d", t);
  u2f: assert property (@(posedge clk) go |-> b[->2]) else $display("U2 fail t=%0d", t);
  u3: cover property (@(posedge clk) go |-> (a[*1:$] intersect (##[2:$] b)))
    $display("U3 pass t=%0d", t);
  u3f: assert property (@(posedge clk) go |-> (a[*1:$] intersect (##[2:$] b)))
    else $display("U3 fail t=%0d", t);
  u4: cover property (@(posedge clk) go |-> (a[*1:$] intersect (##[4:$] b)))
    $display("U4 pass t=%0d", t);
  u4f: assert property (@(posedge clk) go |-> (a[*1:$] intersect (##[4:$] b)))
    else $display("U4 fail t=%0d", t);
  u5: cover property (@(posedge clk) go |-> ##[1:$] c) $display("U5 pass t=%0d", t);
  u5f: assert property (@(posedge clk) go |-> ##[1:$] c) else $display("U5 fail t=%0d", t);
  u6: cover property (@(posedge clk) go |-> (b[->1] and c[->1])) $display("U6 pass t=%0d", t);
  u6f: assert property (@(posedge clk) go |-> (b[->1] and c[->1]))
    else $display("U6 fail t=%0d", t);
  u7: cover property (@(posedge clk) go |-> ((a[*1:$] ##1 !a) and (##[1:$] b)))
    $display("U7 pass t=%0d", t);
  u7f: assert property (@(posedge clk) go |-> ((a[*1:$] ##1 !a) and (##[1:$] b)))
    else $display("U7 fail t=%0d", t);
  u8: cover property (@(posedge clk) a |-> ##[1:$] b) $display("U8 pass t=%0d", t);
  u8f: assert property (@(posedge clk) a |-> ##[1:$] b) else $display("U8 fail t=%0d", t);

  initial begin
    go = 1'b1;
    a = 1'b1;
    for (int k = 1; k <= 12; k++) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
      t = k;
      go = 1'b0;
      a = k <= 5;
      b = k == 3 || k == 8;
    end
    $finish;
  end
endmodule
