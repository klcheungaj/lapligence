// Decision S44-D6: the wait of an instance whose clocking event reads its
// own storage (S44-D5) starts when the new waiter first runs, in the time
// slot of new(), after the constructing process suspends. An event that the
// constructing process itself triggers before it suspends is not sampled;
// the next one is.
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32092): "If a clocking
// event is specified, it defines the event at which coverage points are
// sampled." 19.3 (L32102-32104): "a coverage point is sampled the instant the
// clocking event takes place, as if the process triggering the event were to
// call the built-in sample() method." The text does not say from which
// moment a new instance observes its clocking event.
class Source;
  logic clk = 0;
  bit [1:0] val;
endclass
class Monitor;
  Source src;
  covergroup cg @(posedge src.clk);
    coverpoint src.val;
  endgroup
  function new(Source s);
    src = s;
    cg = new;
  endfunction
endclass
module tb;
  Source s;
  Monitor m;
  initial begin
    s = new;
    m = new(s);
    s.val = 1;
    s.clk = 1;
    #1 s.clk = 0;
    #1 s.val = 2;
    s.clk = 1;
    #1 $display("cg=%0.2f", m.cg.get_inst_coverage());
    $finish;
  end
endmodule
