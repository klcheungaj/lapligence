// Decision S45-D19 (negative, llg extension): a transition set of one range
// list whose repetition can be 1 is rejected whatever the repetition
// operator: (3 [-> 1]), (3 [= 1:2]) and (3 [* 1:3]) are transitions of
// length 0 as well. Compilation fails. Some simulators accept these forms;
// the text does not name them.
//
// IEEE 1800-2009 19.5.1 (SystemVerilog-1800-2009.txt L32735-32737):
// "Transition bin specifications of length 0 shall be illegal. These are
// transition bin specifications containing a trans_set production of a
// single value_range, e.g., (0) or ([0:1]), or a single value_range with a
// repeat_range evaluating to 1, e.g., (0[*1]) or ([0:1][*1])." The examples
// use only [*] and name a repeat_range that evaluates to 1; llg applies the
// rule to every repetition operator and to a range that includes 1 (see
// S45-D11 for the forms the text names).
module tb;
  bit [3:0] v;
  covergroup g;
    cp: coverpoint v { bins t = (3 [-> 1]); }
  endgroup
  g c = new;
  initial begin
    v = 3;
    c.sample();
    $display("%0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
