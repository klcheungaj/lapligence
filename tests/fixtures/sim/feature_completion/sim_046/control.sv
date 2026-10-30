// SIM-046 $coverage_control/_get/_get_max/_save/_merge status values
// (IEEE 1800-2009 40.3.1, Table 40-1, Table 40-2). Derivations: readme.md.
module leaf(input logic clk);
  lp: assert property (@(posedge clk) 1'b1);
endmodule

module mid(input logic clk);
  leaf l(clk);
endmodule

module empty_unit;
endmodule

module tb;
  logic clk = 0;
  integer unknown = 'x;
  mid m1(clk);
  mid m2(clk);
  empty_unit e();
  always #5 clk = ~clk;

  initial begin
    $display("check hier tb %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_HIER, tb));
    $display("check module tb %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_MODULE, tb));
    $display("check module m1.l %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_MODULE, m1.l));
    $display("check hier mid %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_HIER, "mid"));
    $display("check module mid %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_MODULE, "mid"));
    $display("check hier root %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_HIER, $root));
    $display("empty: check %0d start %0d stop %0d reset %0d get %0d max %0d",
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_HIER, e),
             $coverage_control(`SV_COV_START, `SV_COV_ASSERTION, `SV_COV_HIER, e),
             $coverage_control(`SV_COV_STOP, `SV_COV_ASSERTION, `SV_COV_HIER, e),
             $coverage_control(`SV_COV_RESET, `SV_COV_ASSERTION, `SV_COV_HIER, e),
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, e),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, e));
    // Bad arguments: control, type, scope_def, an unknown definition, an
    // instance name given as a string (S46-D6) and unknown bits.
    $display("errors %0d %0d %0d %0d %0d %0d %0d %0d %0d",
             $coverage_control(4, `SV_COV_ASSERTION, `SV_COV_HIER, tb),
             $coverage_control(`SV_COV_CHECK, 19, `SV_COV_HIER, tb),
             $coverage_get(24, `SV_COV_HIER, tb),
             $coverage_get_max(`SV_COV_ASSERTION, 12, tb),
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_MODULE, "nosuch"),
             $coverage_control(`SV_COV_CHECK, `SV_COV_ASSERTION, `SV_COV_MODULE, "m1"),
             $coverage_get_max(unknown, `SV_COV_HIER, tb),
             $coverage_save(19, "never.db"),
             $coverage_control(`SV_COV_STOP, `SV_COV_TOGGLE, 9, tb));
    // No statement or toggle coverage, and no FSM in this design (S46-D5).
    for (int kind = `SV_COV_FSM_STATE; kind <= `SV_COV_TOGGLE; kind++)
      $display("kind %0d: check %0d start %0d stop %0d reset %0d get %0d max %0d save %0d",
               kind,
               $coverage_control(`SV_COV_CHECK, kind, `SV_COV_HIER, $root),
               $coverage_control(`SV_COV_START, kind, `SV_COV_HIER, $root),
               $coverage_control(`SV_COV_STOP, kind, `SV_COV_HIER, $root),
               $coverage_control(`SV_COV_RESET, kind, `SV_COV_HIER, $root),
               $coverage_get(kind, `SV_COV_HIER, $root),
               $coverage_get_max(kind, `SV_COV_HIER, $root),
               $coverage_save(kind, "never.db"));
    #10;
    $display("covered %0d/%0d",
             $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, $root),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, $root));
    // A statement written with a discarded status still acts.
    $coverage_control(`SV_COV_RESET, `SV_COV_ASSERTION, `SV_COV_HIER, m1);
    $display("after reset m1 %0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, $root));
    $display("save %0d", $coverage_save(`SV_COV_ASSERTION, "c.db"));
    $display("merge toggle %0d assertion %0d", $coverage_merge(`SV_COV_TOGGLE, "c.db"),
             $coverage_merge(`SV_COV_ASSERTION, "c.db"));
    $display("after merge %0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, $root));
    $finish;
  end
endmodule
