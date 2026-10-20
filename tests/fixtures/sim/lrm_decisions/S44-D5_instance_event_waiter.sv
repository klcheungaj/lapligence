// Decision S44-D5: a covergroup whose clocking event reads one instance's
// storage (a covergroup argument or a property of the embedding object)
// waits on that event once per instance. The wait belongs to the instance,
// not to the process that called new(): `wait fork` and `disable fork` in
// that process do not see it, and it keeps sampling after the program drops
// every handle to the instance (as in S44-D3). While the instance is
// stopped, its events sample nothing; after start() the next event samples.
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32092-32093): "If a
// clocking event is specified, it defines the event at which coverage points
// are sampled. Because it is in the scope of the covergroup, the clocking
// event can be based on ref arguments of the covergroup." 19.3 (L32102-
// 32104): "a coverage point is sampled the instant the clocking event takes
// place, as if the process triggering the event were to call the built-in
// sample() method." 19.8 (Table 19-5, L33562-33566): stop() "Stops
// collecting coverage information"; start() "Starts collecting coverage
// information". The text does not say which process waits for the event
// of an instance, nor how long it waits. option.per_instance = 1 keeps the
// instance coverage (19.7 Table 19-1) portable; get_coverage() is the
// weighted average of the instances (19.11.3, merge_instances is 0).
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
  Monitor kept, dropped;
  task automatic edge_with(bit [1:0] v);
    #1 s.val = v; s.clk = 1;
    #1 s.clk = 0;
  endtask
  initial begin
    s = new;
    fork
      kept = new(s);
    join_none
    #0 dropped = new(s);
    wait fork;
    $display("wait fork returned at %0t", $time);
    disable fork;
    edge_with(0);
    dropped = null;
    edge_with(1);
    kept.cg.stop();
    edge_with(2);
    kept.cg.start();
    edge_with(3);
    $display("kept=%0.2f type=%0.2f", kept.cg.get_inst_coverage(), kept.cg.get_coverage());
    $finish;
  end
endmodule
