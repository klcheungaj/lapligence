// Decision S45-D5: a bins array `b[N]` whose N, read from a covergroup
// argument, is not positive is a run-time error when the instance is
// constructed; the declaration then has no bins and the simulation continues
// (the run exits with a failure status). A constant N that is not positive is
// a compile-time error.
//
// IEEE 1800-2009 19.5 (SystemVerilog-1800-2009.txt L32415-32418): "To create
// a fixed number of bins for a set of values, a number can be specified
// inside the square brackets. The open_range_list used to specify the set of
// values associated with a bin shall be constant expressions (see 11.2.1),
// instance constants (for classes only), or non-ref arguments to the
// coverage group." The text is silent on a number of bins that is zero or
// negative.
module tb;
  bit [3:0] a;
  covergroup g(int n);
    cp: coverpoint a {
      bins b[n] = {[0:3]};
      bins other = {9};
    }
  endgroup
  g c;
  initial begin
    c = new(0);
    a = 4'd2;
    c.sample();
    $display("after 2: %0.2f", c.get_inst_coverage());
    a = 4'd9;
    c.sample();
    $display("after 9: %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
