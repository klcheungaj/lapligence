// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/db_errors.sv
// SIM-046: a coverage database that is missing, corrupt or saved by a model
// with other covergroups is a located run-time error and changes no coverage;
// a database that cannot be written is a run-time error and leaves no file
// (IEEE 1800-2009 19.9, 40.3.2.5). +save=<f> names the database to write and
// +load=<f> the one to read; -G W=3 narrows the coverpoint, which changes the
// model's covergroups.
module tb #(parameter int W = 4);
  bit [W-1:0] v;
  covergroup g;
    p: coverpoint v;
  endgroup
  g c = new;
  int covered, total;
  real r;
  string f;

  initial begin
    v = 3'd3;
    c.sample();
    if ($value$plusargs("load=%s", f)) $load_coverage_db(f);
    r = g::get_coverage(covered, total);
    $display("type %0d %0d %f", covered, total, r);
    if ($value$plusargs("save=%s", f)) $set_coverage_db_name(f);
    $finish;
  end
endmodule
