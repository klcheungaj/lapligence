// SIM-046 assertion coverage: counters, scopes, control and save/merge
// (IEEE 1800-2009 40.2.2, 40.3.2, 40.5.3). Hand derivations: readme.md.
module unit(input logic clk, input logic a, input logic b, input logic off);
  p: assert property (@(posedge clk) disable iff (off) a |=> b)
    else $display("%0t %m failed", $time);
  c: cover property (@(posedge clk) disable iff (off) a ##1 b);
  s: assert property (@(posedge clk) disable iff (off) (a ##1 a) |=> b)
    else $display("%0t %m failed", $time);
endmodule

package checks;
  function automatic void check(int value);
    pk: assert (value >= 0) else $display("negative %0d", value);
  endfunction
endpackage

module tb;
  logic clk = 0;
  logic a1 = 0, b1 = 0, a2 = 0, b2 = 0, off1 = 0, off2 = 0;
  int x;
  unit u1(clk, a1, b1, off1);
  unit u2(clk, a2, b2, off2);
  always #5 clk = ~clk;
  cs: cover sequence (@(posedge clk) a1 ##1 b1);

  task automatic show(string what);
    $display("%s: tb %0d/%0d u1 %0d/%0d u2 %0d/%0d unit %0d/%0d root %0d/%0d top %0d/%0d",
             what, $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, tb),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, tb),
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, tb.u1),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, tb.u1),
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, u2),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, u2),
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, "unit"),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, "unit"),
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, $root),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, $root),
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_MODULE, tb));
  endtask

  initial begin
    if ($test$plusargs("merge"))
      $display("merge %0d", $coverage_merge(`SV_COV_ASSERTION, "acov.db"));
    show("start");
    // The values of posedge k (time 10k-5) are set at time 10(k-1).
    a1 = 1'b1; a2 = 1'b1;                                          // tick 1
    #10 a1 = 1'b1; b1 = 1'b1; off2 = 1'b1;                         // tick 2
    #10 a1 = 1'b0; b1 = 1'b1; off2 = 1'b0; a2 = 1'b1; b2 = 1'b0;   // tick 3
    #10 a1 = 1'b1; b1 = 1'b0; a2 = 1'b1; b2 = 1'b1;                // tick 4
    #10 a1 = 1'b0; b1 = 1'b0; a2 = 1'b0; b2 = 1'b0;                // tick 5
    $assertkill(0, tb.u2);
    #10 $asserton(0, tb.u2);                                       // tick 6
    show("tick 5");
    $display("stop u1 %0d",
             $coverage_control(`SV_COV_STOP, `SV_COV_ASSERTION, `SV_COV_MODULE, tb.u1));
    #10 a1 = 1'b1; b1 = 1'b0; a2 = 1'b1; b2 = 1'b0;                // tick 7
    #10 a1 = 1'b0; b1 = 1'b1; a2 = 1'b0; b2 = 1'b1;                // tick 8
    #10 show("tick 8");
    $display("start u1 %0d",
             $coverage_control(`SV_COV_START, `SV_COV_ASSERTION, `SV_COV_MODULE, tb.u1));
    for (x = -1; x < 3; x++) begin
      im: assert (x != 1) else $display("im failed for %0d", x);
      ic: cover (x == 2);
      dc: cover #0 (x == 0);
      checks::check(x);
    end
    $assertoff(0, tb.im2);
    repeat (2) begin
      im2: assert (x == 7) else $display("im2 failed for %0d", x);
      $asserton(0, tb.im2);
    end
    #1 show("checks");
    if ($test$plusargs("merge"))
      $display("save %0d", $coverage_save(`SV_COV_ASSERTION, "acov2.db"));
    else
      $display("save %0d", $coverage_save(`SV_COV_ASSERTION, "acov.db"));
    $display("reset u2 %0d",
             $coverage_control(`SV_COV_RESET, `SV_COV_ASSERTION, `SV_COV_HIER, tb.u2));
    show("reset");
    $finish;
  end
endmodule
