// Decision S45-D3: sampling a value of an illegal_bins declaration issues a
// run-time error that ends the simulation: the sampling statement does not
// complete, nothing after it runs, and the final procedures run (llg: an
// error on stderr, exit status 1). An illegal value counts in no other bin.
//
// IEEE 1800-2009 19.5.5 (SystemVerilog-1800-2009.txt L32835-32841): "All
// values or transitions associated with illegal bins are excluded from
// coverage. For state bins, each illegal value is removed from the set of
// values associated with any coverage bin. ... If an illegal value or
// transition occurs, a run-time error is issued. Illegal bins take
// precedence over any other bins, that is, they will result in a run-time
// error even if they are also included in another bin." The text does not
// say whether the error stops the simulation; llg follows the
// industry-standard behaviour of ending it.
module tb;
  bit [3:0] a;
  covergroup g;
    option.per_instance = 1;
    cp: coverpoint a {
      bins low = {[0:7]};
      bins high = {[8:15]};
      illegal_bins bad = {7};
    }
  endgroup
  g c = new;

  final begin
    $display("final block");
  end

  initial begin
    a = 4'd2;
    c.sample();
    $display("after 2: %0.2f", c.get_inst_coverage());
    a = 4'd7;
    c.sample();
    $display("after 7: %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
