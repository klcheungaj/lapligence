// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/transitions.sv
// SIM-045: transition bins (IEEE 1800-2009 19.5.1, 19.5.3-19.5.5): the
// clause's two examples, removal of covered sequences, per-instance history,
// skipped samples, stop/start, bin guards, X samples and wildcard steps.
// Expected counts and percentages are derived in readme.md.
module tb;
  bit [3:0] v;
  // 19.5.1 example: each bin at at_least 1..4, so the number of levels at
  // 100% is the bin's count (up to 4).
  covergroup sg;
    b2_1: coverpoint v { option.at_least = 1; bins b2 = ( 2 [-> 3:5] ); }
    b2_2: coverpoint v { option.at_least = 2; bins b2 = ( 2 [-> 3:5] ); }
    b2_3: coverpoint v { option.at_least = 3; bins b2 = ( 2 [-> 3:5] ); }
    b2_4: coverpoint v { option.at_least = 4; bins b2 = ( 2 [-> 3:5] ); }
    b3_1: coverpoint v { option.at_least = 1; bins b3 = ( 3 [-> 3:5] ); }
    b3_2: coverpoint v { option.at_least = 2; bins b3 = ( 3 [-> 3:5] ); }
    b3_3: coverpoint v { option.at_least = 3; bins b3 = ( 3 [-> 3:5] ); }
    b3_4: coverpoint v { option.at_least = 4; bins b3 = ( 3 [-> 3:5] ); }
    b5_1: coverpoint v { option.at_least = 1; bins b5 = ( 5 [* 3] ); }
    b5_2: coverpoint v { option.at_least = 2; bins b5 = ( 5 [* 3] ); }
    b5_3: coverpoint v { option.at_least = 3; bins b5 = ( 5 [* 3] ); }
    b5_4: coverpoint v { option.at_least = 4; bins b5 = ( 5 [* 3] ); }
    b6_1: coverpoint v { option.at_least = 1; bins b6 = ( 1 => 3 [-> 4:6] => 1 ); }
    b6_2: coverpoint v { option.at_least = 2; bins b6 = ( 1 => 3 [-> 4:6] => 1 ); }
    b6_3: coverpoint v { option.at_least = 3; bins b6 = ( 1 => 3 [-> 4:6] => 1 ); }
    b6_4: coverpoint v { option.at_least = 4; bins b6 = ( 1 => 3 [-> 4:6] => 1 ); }
    b7_1: coverpoint v { option.at_least = 1; bins b7 = ( 1 => 2 [= 3:6] => 5 ); }
    b7_2: coverpoint v { option.at_least = 2; bins b7 = ( 1 => 2 [= 3:6] => 5 ); }
    b7_3: coverpoint v { option.at_least = 3; bins b7 = ( 1 => 2 [= 3:6] => 5 ); }
    b7_4: coverpoint v { option.at_least = 4; bins b7 = ( 1 => 2 [= 3:6] => 5 ); }
  endgroup

  bit [4:1] v_a;
  covergroup cg;
    all: coverpoint v_a {
      bins sa = (4 => 5 => 6), ([7:9],10 => 11,12);
      bins sb[] = (4 => 5 => 6), ([7:9],10 => 11,12);
      bins sc = (12 => 3 [-> 1]);
      bins allother = default sequence;
    }
    p_sa: coverpoint v_a { bins sa = (4 => 5 => 6), ([7:9],10 => 11,12); }
    p_sb: coverpoint v_a { bins sb[] = (4 => 5 => 6), ([7:9],10 => 11,12); }
  endgroup

  covergroup g_removed;
    cp: coverpoint v {
      bins long = (1 => 2 => 3 => 4);
      bins mid = (2 => 3 => 4);
      bins short = (1 => 2);
      bins part = ([1:2] => 3);
      bins arr[] = ([1:2] => 3);
      bins five = (4 => 5);
      ignore_bins skip = (2 => 3);
    }
  endgroup

  covergroup g_inst with function sample(bit [3:0] x);
    cp: coverpoint x { bins t = (1 => 2); }
  endgroup

  covergroup g_skip with function sample(bit [3:0] x, bit en);
    cp: coverpoint x iff (en) { bins t = (1 => 2); }
    guarded: coverpoint x { bins t = (1 => 2) iff (en); }
  endgroup

  covergroup g_x with function sample(logic [3:0] x);
    next: coverpoint x { bins t = (1 => 2); }
    later: coverpoint x { bins t = (1 => 2 [-> 1]); }
  endgroup

  covergroup g_wild with function sample(bit [1:0] x);
    cp: coverpoint x { wildcard bins w = (2'b1? => 2'b0?); bins other = (0 => 0); }
  endgroup

  sg s = new;
  cg c = new;
  g_removed removed = new;
  g_inst ia = new, ib = new;
  g_skip skip = new, stopped = new;
  g_x gx = new;
  g_wild wild = new;
  bit [3:0] sg_values[18] = '{1, 4, 3, 2, 3, 3, 2, 2, 3, 2, 3, 1, 5, 5, 5, 5, 5, 5};
  bit [4:1] cg_values[9] = '{4, 5, 7, 11, 8, 12, 2, 2, 3};
  bit [3:0] removed_values[6] = '{1, 2, 3, 4, 1, 3};

  function automatic int levels(real l1, real l2, real l3, real l4);
    return int'(l1 == 100.0) + int'(l2 == 100.0) + int'(l3 == 100.0) + int'(l4 == 100.0);
  endfunction

  initial begin
    foreach (sg_values[i]) begin
      v = sg_values[i];
      s.sample();
      $display("sg %0d: b2 %0d b3 %0d b5 %0d b6 %0d b7 %0d", i + 1,
               levels(s.b2_1.get_inst_coverage(), s.b2_2.get_inst_coverage(),
                      s.b2_3.get_inst_coverage(), s.b2_4.get_inst_coverage()),
               levels(s.b3_1.get_inst_coverage(), s.b3_2.get_inst_coverage(),
                      s.b3_3.get_inst_coverage(), s.b3_4.get_inst_coverage()),
               levels(s.b5_1.get_inst_coverage(), s.b5_2.get_inst_coverage(),
                      s.b5_3.get_inst_coverage(), s.b5_4.get_inst_coverage()),
               levels(s.b6_1.get_inst_coverage(), s.b6_2.get_inst_coverage(),
                      s.b6_3.get_inst_coverage(), s.b6_4.get_inst_coverage()),
               levels(s.b7_1.get_inst_coverage(), s.b7_2.get_inst_coverage(),
                      s.b7_3.get_inst_coverage(), s.b7_4.get_inst_coverage()));
    end

    foreach (cg_values[i]) begin
      v_a = cg_values[i];
      c.sample();
    end
    $display("cg %0.2f %0.2f %0.2f", c.all.get_inst_coverage(), c.p_sa.get_inst_coverage(),
             c.p_sb.get_inst_coverage());

    foreach (removed_values[i]) begin
      v = removed_values[i];
      removed.sample();
      if (i == 3) $display("removed %0.2f", removed.get_inst_coverage());
    end
    $display("removed %0.2f", removed.get_inst_coverage());

    ia.sample(1); ib.sample(2); ia.sample(2); ib.sample(1);
    $display("instances %0.2f %0.2f %0.2f", ia.get_inst_coverage(), ib.get_inst_coverage(),
             g_inst::get_coverage());

    skip.sample(1, 1); skip.sample(5, 0); skip.sample(2, 1);
    $display("skip %0.2f %0.2f", skip.cp.get_inst_coverage(), skip.guarded.get_inst_coverage());
    skip.sample(1, 0); skip.sample(2, 1);
    $display("skip %0.2f %0.2f", skip.cp.get_inst_coverage(), skip.guarded.get_inst_coverage());
    stopped.sample(1, 1); stopped.stop(); stopped.start(); stopped.sample(2, 1);
    $display("stopped %0.2f", stopped.cp.get_inst_coverage());
    stopped.sample(1, 1); stopped.sample(2, 1);
    $display("stopped %0.2f", stopped.cp.get_inst_coverage());

    gx.sample(1); gx.sample(4'bx01x); gx.sample(2);
    $display("x %0.2f %0.2f", gx.next.get_inst_coverage(), gx.later.get_inst_coverage());

    wild.sample(3); wild.sample(1);
    $display("wild %0.2f", wild.get_inst_coverage());
    $finish;
  end
endmodule
