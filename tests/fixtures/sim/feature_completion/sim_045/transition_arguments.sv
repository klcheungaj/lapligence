// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/transition_arguments.sv
// SIM-045: transition bins whose values read covergroup arguments (IEEE
// 1800-2009 19.5, 19.5.1, 19.5.6), resolved per instance when it is
// constructed; merged type coverage of such bins (19.11.3). Expected values
// are derived in readme.md.
module tb;
  bit [3:0] y;
  covergroup cg(int lo, int hi, int big);
    option.per_instance = 1;
    p_tr: coverpoint y { bins tr = (lo => 3); }
    p_rng: coverpoint y { bins rng = ([lo:hi] => 5); }
    p_two: coverpoint y { bins two = (lo => hi), (hi => lo); }
    p_out: coverpoint y { bins out = (big => 1); bins c = (7 => 8); }
    p_part: coverpoint y { bins part = ([lo:big] => 1); }
    p_rep: coverpoint y { bins rep = (lo [* 2] => hi); }
    p_goto: coverpoint y { bins g = (lo [-> 2] => hi); }
    p_mix: coverpoint y { bins m = (lo => 8); bins k = (8 => 1); }
  endgroup
  covergroup cm(int lo, int big);
    type_option.merge_instances = 1;
    p: coverpoint y { bins t = (lo => 3); bins o = (big => 1); bins c = (7 => 8); bins n = (lo => 9); }
  endgroup
  cg a = new(2, 4, 20), b = new(4, 6, 30);
  cm m1 = new(2, 20), m2 = new(4, 3);
  int vals[] = '{2, 3, 5, 4, 2, 7, 8, 3, 1, 4, 4, 6, 2, 2, 4, 8, 1, 4, 6};
  initial begin
    foreach (vals[i]) begin
      y = vals[i];
      a.sample();
      b.sample();
      m1.sample();
      m2.sample();
    end
    $display("a tr %0.2f rng %0.2f two %0.2f out %0.2f part %0.2f rep %0.2f goto %0.2f mix %0.2f all %0.2f",
      a.p_tr.get_inst_coverage(), a.p_rng.get_inst_coverage(), a.p_two.get_inst_coverage(),
      a.p_out.get_inst_coverage(), a.p_part.get_inst_coverage(), a.p_rep.get_inst_coverage(),
      a.p_goto.get_inst_coverage(), a.p_mix.get_inst_coverage(), a.get_inst_coverage());
    $display("b tr %0.2f rng %0.2f two %0.2f out %0.2f part %0.2f rep %0.2f goto %0.2f mix %0.2f all %0.2f",
      b.p_tr.get_inst_coverage(), b.p_rng.get_inst_coverage(), b.p_two.get_inst_coverage(),
      b.p_out.get_inst_coverage(), b.p_part.get_inst_coverage(), b.p_rep.get_inst_coverage(),
      b.p_goto.get_inst_coverage(), b.p_mix.get_inst_coverage(), b.get_inst_coverage());
    $display("merged %0.2f", m1.get_coverage());
    $finish;
  end
endmodule
