// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/auto_bin_max_write.sv
// SIM-046 (S46-D16): a procedural write of option.auto_bin_max after the
// covergroup is constructed is ignored with a lowering warning, in every
// assignment form; the right-hand side is still evaluated. The automatic bins
// keep the count fixed by the definition (19.7, 19.5.2).
module tb;
  bit [3:0] x, y;
  int calls;
  function automatic int f(int v);
    calls++;
    return v;
  endfunction
  covergroup g;
    option.per_instance = 1;
    option.auto_bin_max = 8;
    cp: coverpoint x { option.auto_bin_max = 4; }
    cq: coverpoint y;
  endgroup
  g a;
  initial begin
    a = new;
    $display("decl cp=%0d cq=%0d group=%0d", a.cp.option.auto_bin_max,
             a.cq.option.auto_bin_max, a.option.auto_bin_max);
    a.cp.option.auto_bin_max = f(2);
    a.option.auto_bin_max = 0;
    a.cq.option.auto_bin_max += 1;
    a.cq.option.auto_bin_max++;
    --a.cp.option.auto_bin_max;
    if (calls == 1) begin
      a.cp.option.auto_bin_max = 16;
    end
    $display("after cp=%0d cq=%0d group=%0d calls=%0d", a.cp.option.auto_bin_max,
             a.cq.option.auto_bin_max, a.option.auto_bin_max, calls);
    x = 0; y = 0; a.sample();
    $display("cp %0.2f cq %0.2f", a.cp.get_inst_coverage(), a.cq.get_inst_coverage());
    #2 $display("after nba cp=%0d", a.cp.option.auto_bin_max);
    $finish;
  end
  initial #1 a.cp.option.auto_bin_max <= 1;
endmodule
