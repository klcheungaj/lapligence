// Decision S44-D2: the automatic bins of a signed coverpoint split its values
// in numeric order, from the most negative value up.
//
// IEEE 1800-2009 19.5.2 (SystemVerilog-1800-2009.txt L32751-32754): "If the
// number of automatic bins is smaller than the number of possible values
// (N < 2M ), then the 2M values are uniformly distributed in the N bins. If
// the number of values, 2M, is not divisible by N, then the last bin will
// include the additional remaining items. For example, if M is 3 and N is 3,
// then the eight possible values are distributed as follows: <0:1>, <2:3>,
// <4,5,6,7>." The example is unsigned; the text does not order the values
// of a signed coverpoint. llg orders them numerically, so a 3-bit signed
// coverpoint with three bins has <-4:-3>, <-2:-1> and <0,1,2,3>.
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  bit signed [2:0] s;
  bit [2:0] u;
  covergroup cg;
    option.per_instance = 1;
    cs: coverpoint s { option.auto_bin_max = 3; }
    cu: coverpoint u { option.auto_bin_max = 3; }
  endgroup
  cg c = new;
  initial begin
    s = 0;
    u = 0;
    c.sample();
    s = 2;
    u = 2;
    c.sample();
    $display("signed=%0.2f unsigned=%0.2f", c.cs.get_inst_coverage(), c.cu.get_inst_coverage());
    s = -3;
    c.sample();
    $display("signed=%0.2f", c.cs.get_inst_coverage());
    $finish;
  end
endmodule
