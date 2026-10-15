// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/instances.sv
// SIM-044: instances of one covergroup collect independently; arguments set per-instance options; sample(), start(), stop() and the coverage queries (IEEE 1800-2009 19.3, 19.5.2, 19.7, 19.8, 19.9, 19.11).
module tb;
  bit [2:0] v;
  covergroup cg(int lim, int w);
    option.auto_bin_max = lim;
    option.weight = w;
    cp: coverpoint v;
  endgroup
  cg a, b, c;
  initial begin
    $display("before type=%0.2f total=%0.2f", cg::get_coverage(), $get_coverage());
    a = new(8, 1);
    b = new(2, 3);
    c = new(8, 0);
    $display("unsampled a=%0.2f b=%0.2f c=%0.2f type=%0.2f", a.get_inst_coverage(),
             b.get_inst_coverage(), c.get_inst_coverage(), cg::get_coverage());
    v = 1;
    a.sample();
    $display("a=%0.2f b=%0.2f", a.get_inst_coverage(), b.get_inst_coverage());
    v = 5;
    b.sample();
    $display("a=%0.2f b=%0.2f", a.get_inst_coverage(), b.get_inst_coverage());
    $display("type=%0.3f via-instance=%0.3f point=%0.3f", cg::get_coverage(), a.get_coverage(),
             a.cp.get_coverage());
    b.stop();
    v = 0;
    b.sample();
    b.start();
    a.cp.stop();
    v = 2;
    a.sample();
    a.cp.start();
    v = 3;
    a.sample();
    $display("a=%0.2f b=%0.2f point=%0.2f", a.get_inst_coverage(), b.get_inst_coverage(),
             a.cp.get_inst_coverage());
    a = null;
    $display("type=%0.2f total=%0.2f", cg::get_coverage(), $get_coverage());
    $finish;
  end
endmodule
