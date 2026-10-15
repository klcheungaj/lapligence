// Decision S45-D3: sampling a value of an illegal_bins declaration issues a
// run-time error and the simulation continues; the value counts in no bin,
// including another bin that lists it.
//
// IEEE 1800-2009 19.5.5 (SystemVerilog-1800-2009.txt L32835-32841): "All
// values or transitions associated with illegal bins are excluded from
// coverage. For state bins, each illegal value is removed from the set of
// values associated with any coverage bin. ... If an illegal value or
// transition occurs, a run-time error is issued. Illegal bins take
// precedence over any other bins, that is, they will result in a run-time
// error even if they are also included in another bin." The text does not
// say whether the error stops the simulation; llg reports it like $error and
// continues (the run then exits with a failure status).
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a {
      bins low = {[0:7]};
      bins seven = {7};
      illegal_bins bad = {7};
    }
  endgroup
  g c = new;
  initial begin
    a = 4'd7;
    c.sample();
    $display("after 7: %0.2f", c.get_inst_coverage());
    a = 4'd2;
    c.sample();
    $display("after 2: %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
