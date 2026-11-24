// FIN-002 testbench scenario 5: two generated channels on different clocks,
// each an interface instance with a clocking block, a modport and a timed
// task, feeding an RTL accumulator through an interface port. A class
// monitor (parameterized by its virtual interface type) waits on an edge,
// on a clocking block event and in a level-sensitive wait through one
// handle, which the test rebinds to the other channel while all three
// waits are pending. The monitor folds what it sees through a DPI-C
// function and a VPI application (shared_force_vpi.c) reads the same
// interface variables by name. Every expected line is derived in readme.md.
`timescale 1ns / 1ns

interface chan_if (
    input logic clk
);
  logic valid = 1'b0;
  logic [7:0] data = '0;
  int beats = 0;

  clocking cb @(posedge clk);
    input valid, data;
  endclocking

  modport rtl(input clk, valid, data);

  task automatic send(input logic [7:0] value);
    @(negedge clk);
    valid = 1'b1;
    data  = value;
    @(negedge clk);
    valid = 1'b0;
    beats++;
  endtask
endinterface

module sink (
    chan_if.rtl c,
    input logic rst,
    output logic [15:0] sum
);
  always_ff @(posedge c.clk)
    if (rst) sum <= '0;
    else if (c.valid) sum <= sum + 16'(c.data);
endmodule

package mon_pkg;
  import "DPI-C" pure function int c_fold(
    input int folded,
    input int value
  );

  class monitor #(
      type VIF = int
  );
    VIF vif;
    int folded;
    int edges;
    int ticks;
    function new(VIF vif);
      this.vif = vif;
    endfunction
    // Edge wait: sees the interface variables as they are after the edge.
    task run_edges();
      forever begin
        @(posedge vif.clk);
        edges++;
        if (vif.valid) begin
          folded = c_fold(folded, int'(vif.data));
          $display("[%0t] edge monitor: %0d", $time, vif.data);
        end
      end
    endtask
    // Clocking wait: sees the values sampled before the edge.
    task run_clocking();
      forever begin
        @(vif.cb);
        ticks++;
        if (vif.cb.valid) $display("[%0t] clocking monitor: %0d", $time, vif.cb.data);
      end
    endtask
    task wait_beats(int count);
      wait (vif.beats >= count);
      $display("[%0t] %0d beats reached", $time, count);
    endtask
  endclass
endpackage

module tb;
  import mon_pkg::*;
  logic [1:0] clk = 2'b00;
  logic rst = 1'b1;
  logic [15:0] sums[2];
  always #5 clk[0] = ~clk[0];
  always #3 clk[1] = ~clk[1];

  for (genvar i = 0; i < 2; i++) begin : ch
    chan_if bus (clk[i]);
    sink s (
        .c  (bus),
        .rst(rst),
        .sum(sums[i])
    );
  end

  virtual chan_if vifs[2];
  monitor #(virtual chan_if) mon;

  initial begin
    vifs[0] = ch[0].bus;
    vifs[1] = ch[1].bus;
    mon = new(vifs[0]);
    fork
      mon.run_edges();
      mon.run_clocking();
      mon.wait_beats(3);
      for (int i = 1; i <= 3; i++) vifs[0].send(8'(10 + i));
      for (int i = 1; i <= 6; i++) vifs[1].send(8'(20 + i));
    join_none
    // Channel 0 clock is low and channel 1 clock is high at 22: the
    // rebinding is a rising edge of `vif.clk`.
    #7 rst = 1'b0;
    #15 mon.vif = vifs[1];
    $display("[%0t] monitor moved to channel 1: data=%0d vpi=%0d", $time, mon.vif.data, $vget(
             "tb.ch[1].bus.data"));
    // Both clocks are low at 42 (channel 0 fell at 40, channel 1 at 42).
    #20 mon.vif = vifs[0];
    $display("[%0t] monitor moved to channel 0: data=%0d vpi=%0d", $time, mon.vif.data, $vget(
             "tb.ch[0].bus.data"));
    #30;
    $display("[%0t] edges=%0d ticks=%0d folded=%0d", $time, mon.edges, mon.ticks, mon.folded);
    $display("sums=%0d,%0d beats=%0d,%0d vpi sums=%0d,%0d", sums[0], sums[1], vifs[0].beats,
             vifs[1].beats, $vget("tb.ch[0].s.sum"), $vget("tb.ch[1].s.sum"));
    $finish;
  end
endmodule
