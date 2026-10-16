// SIM-099 step 2: bit, part-select and indexed part-select writes, blocking
// and nonblocking, made by a class through a virtual interface notify
// exactly the waiters, continuous assignments and VPI value-change
// callbacks whose operand changed (selected_writes.c watches tb.seen and
// tb.low). An equal-value write notifies nobody. Lines of one time step may
// appear in any order (IEEE 1800-2009 4.7); the test compares them as sets.
`timescale 1ns / 1ns

interface sel_if;
  logic [7:0] data;
endinterface

class writer_c;
  virtual sel_if v;
  function new(virtual sel_if v);
    this.v = v;
  endfunction
  task run();
    #1 v.data = 8'h00;
    #1 v.data[3] = 1'b1;
    #1 v.data[7:4] = 4'ha;
    #1 v.data[3] = 1'b1;
    #1 v.data[1+:2] = 2'b11;
    #1 v.data[0] <= 1'b1;
    #1 v.data[7:4] <= 4'h5;
    #1 v.data <= 8'h0f;
  endtask
endclass

module tb;
  sel_if bus ();
  logic [7:0] seen;
  logic [3:0] low;
  assign seen = bus.data;
  assign low = bus.data[3:0];
  writer_c w;

  always @(bus.data) $display("[%0t] data -> %h", $time, bus.data);
  always @(bus.data[3]) $display("[%0t] data[3] -> %b", $time, bus.data[3]);

  initial begin
    w = new(bus);
    w.run();
    #1 $display("[%0t] final data=%h low=%h", $time, bus.data, low);
    $finish;
  end
endmodule
