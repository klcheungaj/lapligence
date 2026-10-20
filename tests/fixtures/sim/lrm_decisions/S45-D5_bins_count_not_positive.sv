// Decision S45-D5: a bins array `b[N]` whose N, read from a covergroup
// argument, is not positive is a run-time error when the instance is
// constructed, and it ends the simulation: nothing after the construction
// runs, and the final procedures run (llg: an error on stderr, exit status
// 1). A constant N that is not positive is a compile-time error.
//
// IEEE 1800-2009 19.5 (SystemVerilog-1800-2009.txt L32415-32418): "To create
// a fixed number of bins for a set of values, a number can be specified
// inside the square brackets. The open_range_list used to specify the set of
// values associated with a bin shall be constant expressions (see 11.2.1),
// instance constants (for classes only), or non-ref arguments to the
// coverage group." The text is silent on a number of bins that is zero or
// negative; llg follows the industry-standard behaviour.
module tb;
  bit [3:0] a;
  covergroup g(int n);
    option.per_instance = 1;
    cp: coverpoint a {
      bins b[n] = {[0:3]};
      bins other = {9};
    }
  endgroup
  g c;

  final begin
    $display("final block");
  end

  initial begin
    c = new(2);
    a = 4'd9;
    c.sample();
    $display("n=2 after 9: %0.2f", c.get_inst_coverage());
    c = new(0);
    $display("after new(0)");
    $finish;
  end
endmodule
