// Decision S45-D15: a value in overlapping coverpoint bins gives the cross
// one product per combination of those bins, and each product counts; a
// user cross bin still counts at most once per sample, however many of its
// products the sample matches.
//
// IEEE 1800-2009 19.6 (SystemVerilog-1800-2009.txt L33051-33054): "A cross
// coverage bin associates a name and a count with a set of cross products.
// The count of the bin is incremented every time any of the cross products
// match". 19.5 L32404-32406: "If the bin designates a set of values, the
// count is incremented every time the coverage point matches one of the
// values in the set." The text does not say whether one sample matching two
// products of one cross bin counts once or twice; llg counts once, like a
// value bin.
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  bit [1:0] a, b;
  covergroup g;
    option.per_instance = 1;
    ca: coverpoint a { bins lo = {[0:1]}; bins mid = {[1:2]}; }
    cb: coverpoint b { bins any = {[0:3]}; }
    ab: cross ca, cb;
    u: cross ca, cb { option.at_least = 2; bins all = binsof(ca); }
  endgroup
  g c = new;
  initial begin
    a = 1; b = 0; c.sample();
    $display("after 1: %0.2f %0.2f", c.ab.get_inst_coverage(), c.u.get_inst_coverage());
    a = 0; b = 0; c.sample();
    $display("after 0: %0.2f %0.2f", c.ab.get_inst_coverage(), c.u.get_inst_coverage());
    $finish;
  end
endmodule
