// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/db_cross.sv
// SIM-046: cross bins and two instances survive the coverage database
// (IEEE 1800-2009 19.9, 19.11.3). The test runs the model three times in
// one directory: +first saves x.db, +second loads it, samples more and
// saves y.db, +third loads y.db and then merges instances. Expected values
// are derived in readme.md.
module tb;
  bit [1:0] a, b;
  covergroup g;
    pa: coverpoint a { bins lo = {[0:1]}; bins hi = {[2:3]}; }
    pb: coverpoint b { bins z = {0}; bins nz = {[1:3]}; }
    x: cross pa, pb { ignore_bins hz = binsof(pa.hi) && binsof(pb.z); }
  endgroup
  g one = new;
  g two = new;
  int c, t;
  real r;

  task automatic report(string label);
    r = g::get_coverage(c, t);
    $display("%s g %0d %0d %f", label, c, t, r);
    r = one.get_inst_coverage(c, t);
    $display("%s one %0d %0d %f", label, c, t, r);
    r = one.x.get_inst_coverage(c, t);
    $display("%s one.x %0d %0d %f", label, c, t, r);
    r = two.get_inst_coverage(c, t);
    $display("%s two %0d %0d %f", label, c, t, r);
    r = g::x::get_coverage(c, t);
    $display("%s g.x %0d %0d %f", label, c, t, r);
  endtask

  task automatic hit(g inst, bit [1:0] va, bit [1:0] vb);
    a = va;
    b = vb;
    inst.sample();
  endtask

  initial begin
    if ($test$plusargs("first")) begin
      hit(one, 0, 0);
      hit(one, 2, 1);
      hit(two, 1, 3);
      report("first");
      $set_coverage_db_name("x.db");
    end
    if ($test$plusargs("second")) begin
      $load_coverage_db("x.db");
      report("loaded");
      hit(two, 3, 2);
      report("second");
      $set_coverage_db_name("y.db");
    end
    if ($test$plusargs("third")) begin
      $load_coverage_db("y.db");
      report("third");
      g::type_option.merge_instances = 1;
      report("merged");
    end
    $finish;
  end
endmodule
