// Decision S45-D1: a bins array `b[N]` with fewer values than bins gives each
// value its own bin, in the order listed; the remaining bins are empty and
// excluded from coverage.
//
// IEEE 1800-2009 19.5 (SystemVerilog-1800-2009.txt L32422-32426): "If a fixed
// number of bins is specified and that number is smaller than the specified
// number of values, then the possible bin values are uniformly distributed
// among the specified bins. The first 'n' specified values are assigned to
// the first bin, the next 'n' specified values are assigned to the next bin,
// etc." L32430-32431: "If the number of bins exceeds the number of values,
// then some of the bins will be empty." 19.11.1 L33858-33860: "If there is no
// value or transition associated with a bin, the bin is ignored and shall not
// contribute to the coverage computation." The text does not say which bins
// are empty; llg assigns one value per bin from the first bin on.
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  bit [3:0] a;
  covergroup g;
    option.per_instance = 1;
    cp: coverpoint a { bins b[4] = {5, 9}; }
  endgroup
  g c = new;
  initial begin
    a = 4'd5;
    c.sample();
    $display("after 5: %0.2f", c.get_inst_coverage());
    a = 4'd9;
    c.sample();
    $display("after 9: %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
