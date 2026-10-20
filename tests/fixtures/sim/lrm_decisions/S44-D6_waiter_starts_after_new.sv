// Decision S44-D6: the wait of an instance whose clocking event reads its
// own storage (S44-D5) starts inside new(): the new waiter arms its event
// before new() returns, so an event that the constructing process triggers
// right after new(), before it suspends, is sampled. The event expression is
// evaluated when the wait arms, so its handles must be valid at new().
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32092): "If a clocking
// event is specified, it defines the event at which coverage points are
// sampled." 19.3 (L32102-32104): "a coverage point is sampled the instant the
// clocking event takes place, as if the process triggering the event were to
// call the built-in sample() method." The text does not say from which
// moment a new instance observes its clocking event; llg follows
// industry-standard behaviour. option.per_instance = 1 keeps the instance
// coverage (19.7 Table 19-1) portable.
class Source;
  logic clk = 0;
  bit [1:0] val;
endclass
class Monitor;
  Source src;
  covergroup cg @(posedge src.clk);
    option.per_instance = 1;
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
