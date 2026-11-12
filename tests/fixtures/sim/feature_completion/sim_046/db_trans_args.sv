// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/db_trans_args.sv
// SIM-046: a saved record whose transition values read covergroup
// arguments keeps its bins when a later run loads it (IEEE 1800-2009 19.5,
// 19.9). The test runs the model twice in one directory: +first saves t.db,
// +second loads it without constructing an instance. Expected values are
// derived in readme.md.
module tb;
  bit [3:0] y;
  covergroup cm(int lo, int hi);
    p: coverpoint y { bins t = (lo => 3); bins o = (hi => 1); bins c = (7 => 8); }
  endgroup
  cm m1;
  int c, t;
  real r;
  int vals[] = '{2, 3, 7, 8};
  initial begin
    if ($test$plusargs("first")) begin
      m1 = new(2, 9);
      foreach (vals[i]) begin
        y = vals[i];
        m1.sample();
      end
      r = cm::get_coverage(c, t);
      $display("first %0d %0d %f", c, t, r);
      $set_coverage_db_name("t.db");
    end
    if ($test$plusargs("second")) begin
      $load_coverage_db("t.db");
      r = cm::get_coverage(c, t);
      $display("loaded %0d %0d %f", c, t, r);
    end
    $finish;
  end
endmodule
