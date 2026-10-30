// SIM-046 statement and toggle coverage in the coverage database (IEEE
// 1800-2009 40.3.2.4, 40.3.2.5; decisions S46-D13 and S46-D14):
// $coverage_save writes the statement or the toggle section and
// $coverage_merge adds a saved run's statement counts and bit sightings and
// changes. A section saved by a model with other signals is an error that
// changes nothing; the other section still merges. The test runs the model
// three times in one directory (+up, +merge, then -G W=3 with +merge).
// Derivations: readme.md.
module tb;
  parameter int W = 2;
  logic [W-1:0] v = '0;
  string tag = "a";
  initial begin
    void'($value$plusargs("tag=%s", tag));
    if ($test$plusargs("merge"))
      $display("merge %0d %0d", $coverage_merge(`SV_COV_STATEMENT, "a.s.db"),
               $coverage_merge(`SV_COV_TOGGLE, "a.t.db"));
    $display("start %0d/%0d %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
             $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    if ($test$plusargs("up")) v = '1;
    #1 v = '0;
    $display("end %0d/%0d %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
             $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    $display("save %0d %0d", $coverage_save(`SV_COV_STATEMENT, {tag, ".s.db"}),
             $coverage_save(`SV_COV_TOGGLE, {tag, ".t.db"}));
    $finish;
  end
endmodule
