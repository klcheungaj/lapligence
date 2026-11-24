// FIN-002 testbench scenario 1: a generated pipeline (clocked, combinational
// and latch logic) behind an interface with a clocking block and modports,
// driven and monitored by classes through a virtual interface. A VPI
// application (shared_force_vpi.c) forces, releases and deposits
// variables the clocking block samples and a concurrent assertion clocked by
// the clocking block checks. Every expected line is derived in readme.md.
`timescale 1ns / 1ns

module stage #(
    parameter int K = 1
) (
    input  logic       clk,
    input  logic       rst,
    input  logic       en,
    input  logic [7:0] d,
    output logic [7:0] q
);
  logic [7:0] nxt;
  always_comb nxt = d + 8'(K);
  always_ff @(posedge clk)
    if (rst) q <= '0;
    else if (en) q <= nxt;
endmodule

module pipe #(
    parameter int N = 3
) (
    input  logic       clk,
    input  logic       rst,
    input  logic       en,
    input  logic [7:0] din,
    output logic [7:0] dout,
    output logic [7:0] held
);
  logic [7:0] tap[N+1];
  assign tap[0] = din;
  for (genvar i = 0; i < N; i++) begin : g
    stage #(.K(i + 1)) s (
        .clk(clk),
        .rst(rst),
        .en (en),
        .d  (tap[i]),
        .q  (tap[i+1])
    );
  end
  assign dout = tap[N];
  // Transparent while the clock is low.
  always_latch if (!clk) held = dout;
endmodule

interface bus_if (
    input logic clk
);
  logic rst;
  logic en;
  logic [7:0] din;
  logic [7:0] dout;
  logic [7:0] held;
  int fails[$];

  clocking cb @(posedge clk);
    default input #1step output #2;
    output rst, en, din;
    input dout, held;
  endclocking

  modport tb(clocking cb);
  modport rtl(input clk, rst, en, din, output dout, held);

  // Three stages add 1 + 2 + 3 to an accepted input.
  property p_sum;
    logic [7:0] v;
    @(cb) disable iff (rst) (en, v = din) |-> ##3 (dout == v + 8'd6);
  endproperty
  a_sum :
  assert property (p_sum)
  else begin
    fails.push_back(int'($time));
    $display("[%0t] a_sum failed", $time);
  end
endinterface

package tb_pkg;
  class driver;
    virtual bus_if.tb vif;
    function new(virtual bus_if.tb vif);
      this.vif = vif;
    endfunction
    task reset(int cycles);
      for (int i = 0; i < cycles; i++) @(vif.cb);
      vif.cb.rst <= 1'b0;
    endtask
    task send(logic [7:0] v);
      @(vif.cb);
      vif.cb.en  <= 1'b1;
      vif.cb.din <= v;
    endtask
  endclass

  class monitor;
    virtual bus_if.tb vif;
    int samples;
    function new(virtual bus_if.tb vif);
      this.vif = vif;
    endfunction
    task run();
      forever begin
        @(vif.cb);
        samples++;
        $display("[%0t] mon dout=%0d held=%0d", $time, vif.cb.dout, vif.cb.held);
      end
    endtask
  endclass
endpackage

module tb;
  import tb_pkg::*;
  logic clk = 1'b0;
  always #5 clk = ~clk;

  bus_if bus (clk);
  pipe #(
      .N(3)
  ) dut (
      .clk (bus.clk),
      .rst (bus.rst),
      .en  (bus.en),
      .din (bus.din),
      .dout(bus.dout),
      .held(bus.held)
  );

  driver  drv;
  monitor mon;

  initial begin
    bus.rst = 1'b1;
    bus.en  = 1'b0;
    bus.din = '0;
    drv = new(bus);
    mon = new(bus);
    $vwatch("tb.bus.dout");
    fork
      mon.run();
    join_none
    drv.reset(2);
    for (int i = 1; i <= 12; i++) drv.send(8'(10 * i));
    repeat (4) @(bus.cb);
    #1;
    $display("[%0t] samples=%0d failures=%0d", $time, mon.samples, bus.fails.size());
    foreach (bus.fails[i]) $display("  failure %0d at %0d", i, bus.fails[i]);
    $vreport;
    $finish;
  end

  // The VPI application disturbs the run between clock edges.
  initial begin
    #58 $vforce("tb.bus.dout", 99);
    $display("[%0t] forced bus.dout: bus=%0d dut=%0d", $time, bus.dout, dut.dout);
    #10 $vrelease("tb.bus.dout");
    $display("[%0t] released bus.dout: bus=%0d dut=%0d", $time, bus.dout, dut.dout);
    #10 $vforce("tb.dut.g[1].s.q", 200);
    $display("[%0t] forced stage 1: q=%0d nxt2=%0d", $time, dut.g[1].s.q, $vget("tb.dut.g[2].s.nxt"));
    #10 $vrelease("tb.dut.g[1].s.q");
    $display("[%0t] released stage 1: q=%0d", $time, dut.g[1].s.q);
    #20 $vput("tb.dut.g[0].s.q", 5);
    $display("[%0t] deposited stage 0: q=%0d nxt1=%0d", $time, dut.g[0].s.q, $vget(
             "tb.dut.g[1].s.nxt"));
  end
endmodule
