// Decision S45-D11 (negative): a transition set of one value_range with a
// repetition that evaluates to 1 is a transition of length 0 and is
// rejected, like the single value_range itself. Compilation fails.
//
// IEEE 1800-2009 19.5.1 (SystemVerilog-1800-2009.txt L32735-32737):
// "Transition bin specifications of length 0 shall be illegal. These are
// transition bin specifications containing a trans_set production of a
// single value_range, e.g., (0) or ([0:1]), or a single value_range with a
// repeat_range evaluating to 1, e.g., (0[*1]) or ([0:1][*1])." The case holds
// the text's repetition form (0[*1]). Some simulators accept it, which the
// text makes illegal; llg follows the clause text. The other repetition
// operators are S45-D19.
module tb;
  bit [3:0] v;
  covergroup g;
    cp: coverpoint v { bins t = (3 [* 1]); }
  endgroup
  g c = new;
  initial begin
    v = 3;
    c.sample();
    $display("%0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
