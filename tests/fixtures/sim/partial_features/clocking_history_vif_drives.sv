// Synchronous drives through virtual interfaces on two instances of one
// interface, each with its own clock. Each driver waits for its instance's
// clocking block event and then drives, so the drive coincides with that
// clocking event and takes effect on it; a drive issued between events
// waits for the next one. A register written on every clock edge is not a
// clocking event of any block.
//
// IEEE 1800-2009 14.10 (SystemVerilog-1800-2009.txt L19763-19764):
//   "The clocking event of a clocking block is available directly by using
//   the clocking block name, regardless of the actual clocking event used to
//   declare the clocking block."
// IEEE 1800-2009 14.16 (L20140-20147):
//   "Regardless of whether the synchronous drive takes effect on the current
//   clocking event or at some future clocking event as a result of a
//   cycle_delay, the corresponding signal shall be updated at a time after
//   that clocking event as specified by the output skew."
//   "It is possible for a drive statement to execute at a time that is not
//   coincident with its clocking event. Such drive statements shall execute
//   without blocking, but shall perform their drive action as if they had
//   executed at the time of the next clocking event."
// IEEE 1800-2009 25.9 (L44891-44893):
//   "Virtual interface variables may be passed as arguments to tasks,
//   functions, or methods. A single virtual interface variable can thus
//   represent different interface instances at different times throughout
//   the simulation."
//
// Clocks: c1 rises at 5, 15, 25, 35; c2 rises at 7, 21, 35. With an output
// skew of #1, d1's drives at 5 and 15 reach i1.x at 6 and 16, d2's at 7 and
// 21 reach i2.x at 8 and 22, and the drive issued at 24 waits for the rise
// of c2 at 35 and reaches i2.x at 36. `x` is a 2-state variable without an
// initializer, so it starts at 0 and no simulator sees a time-0 change on it.
interface ifc (input bit clk);
  bit [7:0] x;
  clocking cb @(posedge clk);
    output #1 x;
  endclocking
endinterface

class driver;
  virtual ifc v;
  int id;
  function new(virtual ifc v, int id);
    this.v = v;
    this.id = id;
  endfunction
  task run(int n);
    for (int k = 1; k <= n; k++) begin
      @(v.cb);
      v.cb.x <= 8'(id * 10 + k);
    end
  endtask
endclass

module tb;
  bit c1 = 0, c2 = 0;
  int edges = 0;
  ifc i1 (c1);
  ifc i2 (c2);
  always #5 c1 = ~c1;
  always #7 c2 = ~c2;
  always @(posedge c1) edges <= edges + 1;
  always @(i1.x) $display("%0d i1.x=%0d", $time, i1.x);
  always @(i2.x) $display("%0d i2.x=%0d", $time, i2.x);
  initial begin
    driver d1, d2;
    d1 = new(i1, 1);
    d2 = new(i2, 2);
    fork
      d1.run(2);
      d2.run(2);
    join
    #3 d2.v.cb.x <= 8'd99;
    #20 $display("%0d edges=%0d", $time, edges);
    $finish;
  end
endmodule
