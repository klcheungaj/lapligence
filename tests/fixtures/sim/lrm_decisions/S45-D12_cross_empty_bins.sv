// Decision S45-D12: a cross's products are those of the crossed coverpoints'
// coverable bins: a coverpoint bin left without values (here by ignore_bins)
// creates no products, as default, ignored and illegal bins create none.
// A user cross bin is significant (counted in Bu) only while one of its
// products is neither ignored nor illegal. Here `ca` has one coverable bin
// and `cb` two, so the cross has 2 products; `gone` is all ignored. The
// denominator is Bc = 1 (one automatic bin) plus Bu = 0.
//
// IEEE 1800-2009 19.6 (SystemVerilog-1800-2009.txt L33033-33034): "No cross
// coverage bins shall be created for coverpoint bins that are specified as
// default, ignored, or illegal bins." 19.11.2 L33917-33921: "Bj is the
// cardinality (number of bins) of the jth coverpoint being crossed ... Bu is
// the number of significant user-defined cross bins—excluding ignore_bins
// and illegal_bins". 19.11.1 L33858-33860: a bin without values "is excluded
// from both the numerator and the denominator". The text neither counts
// empty coverpoint bins in Bj nor defines "significant"; llg reads both as
// "with something left to cover".
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  bit [1:0] a, b;
  covergroup g;
    option.per_instance = 1;
    ca: coverpoint a {
      bins one = {1};
      bins two = {2};
      ignore_bins ig = {2};
    }
    cb: coverpoint b { bins x = {0}; bins y = {1}; }
    ab: cross ca, cb {
      bins gone = binsof(cb.x);
      ignore_bins skip = binsof(cb.x);
    }
  endgroup
  g c = new;
  initial begin
    a = 1; b = 0; c.sample();
    $display("after 1,0: %0.2f", c.ab.get_inst_coverage());
    a = 1; b = 1; c.sample();
    $display("after 1,1: %0.2f", c.ab.get_inst_coverage());
    $finish;
  end
endmodule
