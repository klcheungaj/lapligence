// SIM-046: $coverage_merge of a statement or FSM section saved by a model
// with other statements or other FSMs is a run-time error that returns
// SV_COV_ERROR and changes nothing (IEEE 1800-2009 40.3.2.4; decisions
// S46-D11, S46-D13, A17-D11). The test runs the model twice in one
// directory: the first run saves s.db and f.db; the second, built with
// -D MORE (one more statement and one more FSM state), merges them.
// Derivations: readme.md.
module tb;
  /* tool state_vector cs */
  logic [1:0] /* tool enum S */ cs = 2'd0;
`ifdef MORE
  parameter [1:0] /* tool enum S */ A = 0, B = 1, C = 2;
`else
  parameter [1:0] /* tool enum S */ A = 0, B = 1;
`endif
  int n = 0, s, f;
  initial begin
    cs = 2'd1;
    #1 n = 1;
`ifdef MORE
    n = 2;
`endif
    if ($test$plusargs("merge")) begin
      s = $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, tb);
      f = $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, tb);
      $display("merge %0d %0d", $coverage_merge(`SV_COV_STATEMENT, "s.db"),
               $coverage_merge(`SV_COV_FSM_STATE, "f.db"));
      $display("before %0d %0d after %0d %0d", s, f,
               $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
               $coverage_get(`SV_COV_FSM_STATE, `SV_COV_MODULE, tb));
    end else
      $display("save %0d %0d", $coverage_save(`SV_COV_STATEMENT, "s.db"),
               $coverage_save(`SV_COV_FSM_STATE, "f.db"));
    $finish;
  end
endmodule
