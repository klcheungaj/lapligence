// Decision S46-D16: a procedural write of option.auto_bin_max after the
// covergroup is constructed is ignored (llg reports a warning when it
// compiles the write). The option keeps the value the definition gave it,
// the automatic bins keep the count fixed at construction, and the
// right-hand side of the write is still evaluated. This holds for every
// assignment form, a nonblocking one, an increment and a decrement, at the
// coverpoint and at the covergroup level. The text makes such a write
// illegal; llg follows industry-standard behaviour, which accepts it.
//
// IEEE 1800-2009 19.7 (SystemVerilog-1800-2009.txt L33340-33344): "Option
// assignment statements in the covergroup definition are evaluated at the
// time that the covergroup is instantiated. The per_instance and
// get_inst_coverage options can only be set in the covergroup definition.
// The auto_bin_max and detect_overlap options can only be set in the
// covergroup or coverpoint definition. Other instance-specific options can be
// assigned procedurally after a covergroup has been instantiated."
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
    $display("after cp=%0d cq=%0d group=%0d calls=%0d", a.cp.option.auto_bin_max,
             a.cq.option.auto_bin_max, a.option.auto_bin_max, calls);
    x = 0; y = 0; a.sample();
    $display("cp %0.2f cq %0.2f", a.cp.get_inst_coverage(), a.cq.get_inst_coverage());
    #2 $display("after nba cp=%0d", a.cp.option.auto_bin_max);
    $finish;
  end
  initial #1 a.cp.option.auto_bin_max <= 1;
endmodule
