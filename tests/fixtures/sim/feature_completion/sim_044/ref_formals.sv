// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/ref_formals.sv
// SIM-044: a ref argument is read at each sample through the variable bound by that instance's new(); an input argument keeps the value it had at new() (IEEE 1800-2009 19.3).
module tb;
  bit [2:0] x, y, seed;
  covergroup g(ref bit [2:0] r, input bit [2:0] i);
    cr: coverpoint r;
    ci: coverpoint i;
  endgroup
  g gx, gy, gx2;
  initial begin
    seed = 6;
    gx = new(x, seed);
    gy = new(y, seed);
    seed = 1;
    gx2 = new(x, seed);
    x = 1;
    y = 2;
    gx.sample();
    gy.sample();
    gx2.sample();
    x = 3;
    gx.sample();
    gx2.sample();
    $display("gx r=%0.2f i=%0.2f", gx.cr.get_inst_coverage(), gx.ci.get_inst_coverage());
    $display("gy r=%0.2f i=%0.2f", gy.cr.get_inst_coverage(), gy.ci.get_inst_coverage());
    $display("gx2 r=%0.2f i=%0.2f", gx2.cr.get_inst_coverage(), gx2.ci.get_inst_coverage());
    $display("type=%0.2f", g::get_coverage());
    $finish;
  end
endmodule
