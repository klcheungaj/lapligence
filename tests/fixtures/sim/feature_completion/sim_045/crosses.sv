// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/crosses.sv
// SIM-045: cross coverage (IEEE 1800-2009 19.6, 19.6.1, 19.6.2, 19.11.2): the
// clause's examples, implicit coverpoints, select operators, ignored
// products, guards and a large sparse cross. Expected percentages are
// derived in readme.md.
module tb;
  int i, j;
  bit [7:0] v_a, v_b;
  bit [1:0] p, a, b;
  bit q, en;
  bit [11:0] wa, wb;

  covergroup ct; option.per_instance = 1;
    coverpoint i { bins i[] = { [0:1] }; }
    coverpoint j { bins j[] = { [0:1] }; }
    x1: cross i, j;
    x2: cross i, j {
      bins i_zero = binsof(i) intersect { 0 };
    }
  endgroup

  covergroup cg; option.per_instance = 1;
    a: coverpoint v_a {
      bins a1 = { [0:63] };
      bins a2 = { [64:127] };
      bins a3 = { [128:191] };
      bins a4 = { [192:255] };
    }
    b: coverpoint v_b {
      bins b1 = {0};
      bins b2 = { [1:84] };
      bins b3 = { [85:169] };
      bins b4 = { [170:255] };
    }
    c: cross a, b {
      bins c1 = ! binsof(a) intersect {[100:200]};
      bins c2 = binsof(a.a2) || binsof(b.b2);
      bins c3 = binsof(a.a1) && binsof(b.b4);
    }
  endgroup

  covergroup g_implicit; option.per_instance = 1;
    cq: coverpoint q;
    px: cross p, cq;
  endgroup

  covergroup g_ignore; option.per_instance = 1;
    ca: coverpoint a;
    cb: coverpoint b;
    plain: cross ca, cb { ignore_bins ig = binsof(ca) intersect {0}; }
    both: cross ca, cb {
      bins one = binsof(cb) intersect {1};
      ignore_bins ig = binsof(ca) intersect {0};
    }
  endgroup

  covergroup g_guard; option.per_instance = 1;
    ca: coverpoint a { bins lo = {[0:1]}; bins hi = {[2:3]}; }
    cb: coverpoint b iff (en) { bins lo = {[0:1]}; bins hi = {[2:3]}; }
    on: cross ca, cb iff (q);
    picked: cross ca, cb {
      bins only = binsof(ca.hi) iff (q);
      bins arr = binsof(ca) intersect {[1:2]} && ! binsof(cb.lo) intersect {0};
    }
  endgroup

  covergroup g_large; option.per_instance = 1;
    ca: coverpoint wa { option.auto_bin_max = 4096; }
    cb: coverpoint wb { option.auto_bin_max = 4096; }
    all: cross ca, cb;
    half: cross ca, cb { ignore_bins low = binsof(ca) intersect {[0:2047]}; }
  endgroup

  ct t = new;
  cg c = new;
  g_implicit gi = new;
  g_ignore ig = new;
  g_guard gg = new;
  g_large gl = new;

  initial begin
    i = 0; j = 0; t.sample();
    i = 1; j = 1; t.sample();
    $display("ct %0.2f %0.2f", t.x1.get_inst_coverage(), t.x2.get_inst_coverage());

    v_a = 10; v_b = 200; c.sample();
    v_a = 200; v_b = 0; c.sample();
    $display("cg %0.2f", c.c.get_inst_coverage());
    v_a = 100; v_b = 50; c.sample();
    $display("cg %0.2f %0.2f", c.c.get_inst_coverage(), c.get_inst_coverage());

    p = 1; q = 0; gi.sample();
    p = 3; q = 1; gi.sample();
    $display("implicit %0.2f %0.2f", gi.px.get_inst_coverage(), gi.get_inst_coverage());

    a = 0; b = 1; ig.sample();
    $display("ignore %0.2f %0.2f", ig.plain.get_inst_coverage(), ig.both.get_inst_coverage());
    a = 1; b = 1; ig.sample();
    $display("ignore %0.2f %0.2f", ig.plain.get_inst_coverage(), ig.both.get_inst_coverage());

    a = 2; b = 1; en = 0; q = 1; gg.sample();
    $display("guard %0.2f %0.2f", gg.on.get_inst_coverage(), gg.picked.get_inst_coverage());
    a = 2; b = 2; en = 1; q = 0; gg.sample();
    $display("guard %0.2f %0.2f", gg.on.get_inst_coverage(), gg.picked.get_inst_coverage());
    a = 1; b = 1; en = 1; q = 1; gg.sample();
    $display("guard %0.2f %0.2f", gg.on.get_inst_coverage(), gg.picked.get_inst_coverage());

    wa = 0; wb = 0; gl.sample();
    wa = 4095; wb = 4095; gl.sample();
    wa = 3000; wb = 7; gl.sample();
    $display("large %0.8f %0.8f", gl.all.get_inst_coverage(), gl.half.get_inst_coverage());
    $finish;
  end
endmodule
