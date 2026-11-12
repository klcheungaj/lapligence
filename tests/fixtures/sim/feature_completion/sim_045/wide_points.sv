// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/wide_points.sv
// SIM-045: coverpoints of 65 to 128 bits (IEEE 1800-2009 19.5, 19.5.1-19.5.4,
// 19.5.6, 19.6): value, array, fixed-count, ignored, wildcard, transition,
// automatic (capped by auto_bin_max), signed and crossed bins. Expected
// values are derived in readme.md.
module tb;
  bit [70:0] x;
  bit signed [70:0] s;
  bit [127:0] q;
  bit signed [99:0] t;
  bit [1:0] n;
  covergroup cg;
    option.per_instance = 1;
    p_val: coverpoint x { bins zero = {0}; bins big = {71'h1 << 70}; bins rng = {[71'h1 << 69 : (71'h1 << 70) - 1]}; bins miss = {7}; }
    p_arr: coverpoint x { bins arr[] = {1, 2, 71'h40_0000_0000_0000_0000}; bins top = {[71'h7f_ffff_ffff_ffff_fff0 : $]}; }
    p_ign: coverpoint x { bins all = {[0:15]}; ignore_bins ig = {5}; bins hi = {[71'h1 << 66 : 71'h1 << 67]}; }
    p_auto: coverpoint x;
    p_sauto: coverpoint s;
    p_cnt: coverpoint x { bins c[4] = {[71'h1 << 68 : (71'h1 << 68) + 7]}; }
    p_tr: coverpoint x { bins t = (71'h1 << 70 => 0); bins u = (0 => 71'h1 << 70); bins v[] = ([1:2] => 71'h1 << 66); }
    p_n: coverpoint n;
    p_x: cross p_val, p_n { bins hi = binsof(p_val) intersect {[71'h1 << 69 : $]}; }
    p_wild: coverpoint x { wildcard bins w = {71'h7f_ffff_ffff_ffff_fffx}; bins o = {3}; }
    p_signed: coverpoint s { bins neg = {[-5:-1]}; bins mn = {71'sh40_0000_0000_0000_0000}; bins pos = {[1:100]}; bins z = {0}; }
    p_q: coverpoint q { bins lo = {[0:128'hffff_ffff_ffff_ffff]}; bins hi = {[128'h1_0000_0000_0000_0000:$]}; bins mx = {128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff}; }
    p_qauto: coverpoint q { option.auto_bin_max = 3; }
    p_t: coverpoint t { bins neg[] = {-3, -2, -1}; bins d = default; }
  endgroup
  cg c = new;
  initial begin
    x = 0; s = -71'sd1; n = 0; q = 0; t = -100'sd2; c.sample();
    x = 71'h1 << 70; s = 71'sh40_0000_0000_0000_0000; n = 1; q = {128{1'b1}}; t = 5; c.sample();
    x = 71'h1 << 69; s = 0; q = 128'h1_0000_0000_0000_0000; t = -100'sd3; c.sample();
    x = 2; c.sample();
    x = 71'h1 << 66; c.sample();
    x = 71'h7f_ffff_ffff_ffff_ffff; q = 128'h5555_0000_0000_0000_0000_0000_0000_0000; c.sample();
    x = 5; c.sample();
    x = (71'h1 << 68) + 6; c.sample();
    x = 71'h1 << 66; c.sample();
    x = 0; c.sample();
    $display("val %0.2f arr %0.2f ign %0.2f auto %0.2f sauto %0.2f cnt %0.2f tr %0.2f",
      c.p_val.get_inst_coverage(), c.p_arr.get_inst_coverage(), c.p_ign.get_inst_coverage(),
      c.p_auto.get_inst_coverage(), c.p_sauto.get_inst_coverage(), c.p_cnt.get_inst_coverage(),
      c.p_tr.get_inst_coverage());
    $display("x %0.2f wild %0.2f signed %0.2f q %0.2f qauto %0.2f t %0.2f all %0.2f",
      c.p_x.get_inst_coverage(), c.p_wild.get_inst_coverage(), c.p_signed.get_inst_coverage(),
      c.p_q.get_inst_coverage(), c.p_qauto.get_inst_coverage(), c.p_t.get_inst_coverage(),
      c.get_inst_coverage());
    $finish;
  end
endmodule
