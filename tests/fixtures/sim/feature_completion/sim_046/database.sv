// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/database.sv
// SIM-046: $set_coverage_db_name saves the run's coverage at the end of the
// run; $load_coverage_db adds a saved run to type coverage (IEEE 1800-2009
// 19.9, 19.11.3). The test runs the model three times in one directory:
// +first saves cov.db, +second loads it, samples more and saves cov2.db,
// +third loads cov2.db twice. Expected values are derived in readme.md.
module tb;
  bit [3:0] v;
  covergroup g (string tag);
    option.name = tag;
    option.at_least = 2;
    p: coverpoint v { bins b[4] = {[0:15]}; }
  endgroup
  covergroup h;
    type_option.merge_instances = 1;
    q: coverpoint v { bins lo = {[0:7]}; bins hi = {[8:15]}; }
    tr: coverpoint v { bins up = (1 => 2); }
  endgroup
  g one = new("one");
  g two = new("two");
  g three;
  h m = new;
  int c, t;
  real r;

  task automatic report(string label);
    r = g::get_coverage(c, t);
    $display("%s g %0d %0d %f", label, c, t, r);
    r = one.get_inst_coverage(c, t);
    $display("%s one %0d %0d %f", label, c, t, r);
    r = h::get_coverage(c, t);
    $display("%s h %0d %0d %f", label, c, t, r);
    $display("%s total %f", label, $get_coverage());
  endtask

  initial begin
    if ($test$plusargs("first")) begin
      v = 4'd0; one.sample();
      v = 4'd5; one.sample();
      v = 4'd12; two.sample();
      v = 4'd13; two.sample();
      v = 4'd1; m.sample();
      v = 4'd2; m.sample();
      report("first");
      $set_coverage_db_name("cov.db");
    end
    if ($test$plusargs("second")) begin
      $load_coverage_db("cov.db");
      report("loaded");
      v = 4'd1; one.sample();
      v = 4'd6; one.sample();
      three = new("three");
      v = 4'd15; three.sample();
      v = 4'd9; m.sample();
      report("second");
      $set_coverage_db_name("cov2.db");
    end
    if ($test$plusargs("third")) begin
      $load_coverage_db("cov2.db");
      report("third");
      $load_coverage_db("cov2.db");
      report("twice");
    end
    $finish;
  end
endmodule
