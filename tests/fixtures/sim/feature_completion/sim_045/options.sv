// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/options.sv
// SIM-045: coverage options (IEEE 1800-2009 19.7, 19.7.1, 19.10, 19.11):
// weights at every level, at_least defaults, options without a numeric
// effect, option.detect_overlap warnings, and merged type coverage with
// option.get_inst_coverage, including the clause's examples. Expected
// percentages are derived in readme.md.
module tb;
  bit [7:0] a;
  bit [1:0] x, y;

  covergroup gw(int w);
    option.weight = 2;
    option.goal = 90;
    option.comment = "weights";
    option.per_instance = 1;
    option.at_least = 2;
    option.cross_num_print_missing = 4;
    cx: coverpoint x { option.weight = w; option.at_least = 1; bins lo = {[0:1]}; bins hi = {[2:3]}; }
    cy: coverpoint y { type_option.weight = 0; option.goal = 50; }
    xy: cross cx, cy { option.weight = 3; option.comment = "cross"; }
  endgroup

  covergroup gt(int l, h);
    type_option.merge_instances = 1;
    coverpoint a { bins b[] = { [l:h] }; }
  endgroup

  covergroup gi(int l, h);
    type_option.merge_instances = 1;
    option.get_inst_coverage = 1;
    coverpoint a { bins b[] = { [l:h] }; }
  endgroup

  covergroup ga(int abm);
    type_option.merge_instances = 1;
    option.auto_bin_max = abm;
    coverpoint a { ignore_bins i = {3}; }
  endgroup

  covergroup gm;
    type_option.merge_instances = 1;
    type_option.weight = 2;
    cx: coverpoint x { type_option.weight = 3; bins lo = {[0:1]}; bins mid = {[1:2]}; }
    tx: coverpoint x { type_option.weight = 0; bins up = (0 => 1); bins any[] = (0 => [1:2]); }
    cy: coverpoint y;
    xy: cross cx, cy;
  endgroup

  covergroup go;
    option.detect_overlap = 1;
    cx: coverpoint x { bins lo = {[0:1]}; bins mid = {[1:2]}; bins top = {3}; ignore_bins skip = {3}; }
    tx: coverpoint x { bins t1 = (0 => 1); bins t2 = (0 => [1:2]); bins t3 = (2 => 2 [-> 1]); }
    quiet: coverpoint y { option.detect_overlap = 0; bins p = {[0:2]}; bins q = {2}; }
  endgroup

  gw w1 = new(1), w2 = new(0);
  gt gv1 = new(0, 1), gv2 = new(1, 2);
  gi gw1 = new(0, 1), gw2 = new(1, 2);
  ga g64 = new(64), g32 = new(32);
  gm m1 = new, m2 = new;
  go o = new;

  initial begin
    x = 1; y = 1; w1.sample(); w2.sample();
    $display("gw %0.2f %0.2f %0.2f %0.2f", w1.cx.get_inst_coverage(), w1.cy.get_inst_coverage(),
             w1.xy.get_inst_coverage(), w1.get_inst_coverage());
    y = 1; w1.sample();
    $display("gw %0.2f %0.2f %0.2f %0.4f", w1.cy.get_inst_coverage(), w1.get_inst_coverage(),
             w2.get_inst_coverage(), gw::get_coverage());

    a = 0; gv1.sample(); gw1.sample();
    a = 1; gv1.sample(); gv2.sample(); gw1.sample(); gw2.sample();
    $display("gt %0.4f %0.2f %0.2f", gt::get_coverage(), gv1.get_inst_coverage(), gv2.get_inst_coverage());
    $display("gi %0.4f %0.2f %0.2f", gi::get_coverage(), gw1.get_inst_coverage(), gw2.get_inst_coverage());

    a = 5; g64.sample(); g32.sample();
    $display("ga %0.4f", ga::get_coverage());

    x = 0; y = 0; m1.sample(); m2.sample();
    x = 1; y = 1; m2.sample();
    $display("gm %0.4f %0.2f %0.2f %0.2f %0.2f", gm::get_coverage(), gm::cx::get_coverage(),
             gm::tx::get_coverage(), gm::xy::get_coverage(), m1.get_inst_coverage());

    x = 0; o.sample();
    $display("go %0.2f", o.get_inst_coverage());
    $display("total %0.4f", $get_coverage());
    $finish;
  end
endmodule
