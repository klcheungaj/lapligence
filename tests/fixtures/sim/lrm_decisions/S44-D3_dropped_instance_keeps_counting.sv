// Decision S44-D3: a covergroup instance outlives the last handle to it and
// to its object: an event-sampled instance keeps sampling, and every instance
// keeps contributing to its type coverage until the end of simulation.
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32103-32104): "a
// coverage point is sampled the instant the clocking event takes place, as if
// the process triggering the event were to call the built-in sample()
// method." 19.11 (L33759-33760): "The cumulative (or type) coverage
// considers the contribution of all instances of a particular coverage item".
// The text does not say what happens when the program drops its last
// reference to an instance or to the object that embeds it; llg keeps the
// instance sampling and counting.
module tb;
  logic clk = 0;
  class Holder;
    bit [1:0] level;
    covergroup cg @(posedge clk);
      coverpoint level;
    endgroup
    function new(bit [1:0] start);
      level = start;
      cg = new;
    endfunction
  endclass
  covergroup plain;
    coverpoint clk;
  endgroup
  Holder h, keep;
  plain p;
  initial begin
    h = new(0);
    keep = new(0);
    #1 clk = 1;
    #1 clk = 0;
    h.level = 3;
    h = null;
    #1 clk = 1;
    #1 clk = 0;
    $display("keep=%0.2f type=%0.2f", keep.cg.get_inst_coverage(), keep.cg.get_coverage());
    p = new;
    p.sample();
    p = null;
    $display("plain type=%0.2f", plain::get_coverage());
    $finish;
  end
endmodule
