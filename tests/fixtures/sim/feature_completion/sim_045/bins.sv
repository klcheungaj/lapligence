// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/bins.sv
// SIM-045: explicit, array, default, ignore, wildcard and guarded bins and
// bins that read covergroup arguments (IEEE 1800-2009 19.5, 19.5.3-19.5.6,
// 19.11.1). Expected percentages are derived in readme.md.
module tb;
  typedef enum bit [1:0] {R, G, B} color_t;
  bit [3:0] a;
  bit [2:0] b;
  bit [7:0] w;
  bit signed [3:0] s;
  bit en;
  color_t color;

  covergroup g_values;
    ca: coverpoint a {
      bins low = {[$:2]};
      bins mid = {[2:5], 9};
      bins high = {[12:$]};
      bins d = default;
    }
  endgroup

  covergroup g_arrays;
    cp: coverpoint a {
      bins each[] = {1, [3:4], 3};
      bins fixed[4] = {[1:10], 1, 4, 7};
      bins few[4] = {14, 15};
    }
  endgroup

  covergroup g_removed;
    cb: coverpoint b {
      option.auto_bin_max = 4;
      ignore_bins ig = {[0:1], [5:6]};
    }
    cq: coverpoint b {
      bins pair[2] = {[0:3]};
      ignore_bins skip = {0, 1};
    }
  endgroup

  covergroup g_wild;
    cw: coverpoint w {
      wildcard bins top = {8'b11??_????};
      wildcard bins odd = {8'b????_???1};
      wildcard bins mid = {[8'b0100_0???:8'b0101_1???]};
    }
  endgroup

  covergroup g_guard;
    cg: coverpoint a {
      bins one = {1} iff (en);
      bins two = {2};
    }
  endgroup

  covergroup g_args(int lo, int hi, int n);
    cp: coverpoint a {
      bins good = {[lo:hi]};
      bins split[n] = {[lo:hi]};
      bins rest = default;
    }
  endgroup

  covergroup g_typed;
    ce: coverpoint color { ignore_bins no_g = {G}; }
    cs: coverpoint s {
      bins neg = {[$:-1]};
      bins pos[] = {[1:3]};
    }
  endgroup

  g_values values = new;
  g_arrays arrays = new;
  g_removed removed = new;
  g_wild wild = new;
  g_guard guard = new;
  g_args args1 = new(2, 5, 2);
  g_args args2 = new(10, 20, 3);
  g_typed typed = new;

  initial begin
    a = 2; values.sample();
    a = 7; values.sample();
    $display("values %0.2f", values.get_inst_coverage());
    a = 13; values.sample();
    $display("values %0.2f", values.get_inst_coverage());

    a = 3; arrays.sample();
    a = 10; arrays.sample();
    a = 15; arrays.sample();
    $display("arrays %0.2f", arrays.get_inst_coverage());
    a = 4; arrays.sample();
    $display("arrays %0.2f", arrays.get_inst_coverage());

    b = 4; removed.sample();
    b = 0; removed.sample();
    $display("removed %0.2f %0.2f %0.2f", removed.cb.get_inst_coverage(),
             removed.cq.get_inst_coverage(), removed.get_inst_coverage());
    b = 3; removed.sample();
    $display("removed %0.2f %0.2f %0.2f", removed.cb.get_inst_coverage(),
             removed.cq.get_inst_coverage(), removed.get_inst_coverage());

    w = 8'hc2; wild.sample();
    w = 8'd7; wild.sample();
    $display("wild %0.2f", wild.get_inst_coverage());
    w = 8'd70; wild.sample();
    $display("wild %0.2f", wild.get_inst_coverage());

    a = 1; en = 0; guard.sample();
    a = 2; guard.sample();
    $display("guard %0.2f", guard.get_inst_coverage());
    a = 1; en = 1; guard.sample();
    $display("guard %0.2f", guard.get_inst_coverage());

    a = 4; args1.sample(); args2.sample();
    $display("args %0.2f %0.2f %0.2f", args1.get_inst_coverage(), args2.get_inst_coverage(),
             g_args::get_coverage());

    color = R; s = -3; typed.sample();
    s = 2; typed.sample();
    $display("typed %0.2f %0.2f", typed.ce.get_inst_coverage(), typed.cs.get_inst_coverage());
    $finish;
  end
endmodule
