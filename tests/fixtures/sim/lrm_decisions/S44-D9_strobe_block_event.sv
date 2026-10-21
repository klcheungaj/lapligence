// Decision S44-D9: type_option.strobe has no effect on a block event: a
// `@@(end f)` sample is taken when f finishes, not in the Postponed region.
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32105-32109): "The strobe
// option (see 19.7.1) can be used to specify that coverage points are sampled
// in the Postponed region, thereby filtering multiple clocking events so that
// only one sample per time slot is taken. The strobe option only applies to
// the scheduling of samples triggered by a clocking event. It shall have no
// effect on procedural calls to the built-in sample() method." 19.7.1
// (L33423-33424): strobe "When true, all samples happen at the end of the
// time slot, like the $strobe system task." A block event is an alternative
// to a clocking event (L32111-32112); the text does not say whether strobe
// applies to it.
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  int n;
  function automatic void f(int a);
    n = a;
  endfunction
  covergroup g @@(end f);
    option.per_instance = 1;
    type_option.strobe = 1;
    coverpoint n { bins one = {1}; bins two = {2}; }
  endgroup
  g c = new;
  initial begin
    f(1);
    n = 2;
    $display("g=%0.2f", c.get_inst_coverage());
    #1 $display("g=%0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
