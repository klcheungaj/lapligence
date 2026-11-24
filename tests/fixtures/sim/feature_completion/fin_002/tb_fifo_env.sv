// FIN-002 testbench scenario 6: a generated FIFO (clocked slots, a
// combinational status and a latched read port) behind an interface with a
// clocking block, driven from a program block by a class environment: a
// mailbox-fed driver, a monitor with an embedded covergroup and a
// parameterized scoreboard whose reference model lives in C behind a
// chandle (tb_fifo_env_dpi.c) and reports through an exported function. A
// VPI application (tb_fifo_env_vpi.c) watches the fill count and, from its
// value-change callback, schedules a delayed write of the `stall` variable
// that the clocking block samples and a timed callback that clears it. An
// overflow assertion clocked by the clocking block triggers an event the
// environment counts. Every expected line is derived in readme.md.
`timescale 1ns / 1ns

module fifo #(
    parameter int DEPTH = 4
) (
    input  logic       clk,
    input  logic       rst,
    input  logic       push,
    input  logic       pop,
    input  logic [7:0] din,
    output logic [7:0] dout,
    output logic       full,
    output logic       empty,
    output logic [2:0] count
);
  logic [7:0] mem[DEPTH];
  logic [1:0] wp;
  logic [1:0] rp;
  for (genvar i = 0; i < DEPTH; i++) begin : slot
    always_ff @(posedge clk) if (push && !full && wp == 2'(i)) mem[i] <= din;
  end
  always_ff @(posedge clk)
    if (rst) begin
      wp <= '0;
      rp <= '0;
      count <= '0;
    end else begin
      if (push && !full) wp <= wp + 2'd1;
      if (pop && !empty) rp <= rp + 2'd1;
      count <= count + 3'(push && !full) - 3'(pop && !empty);
    end
  always_comb begin
    full  = count == 3'(DEPTH);
    empty = count == 3'd0;
  end
  // The read port follows the head slot while the clock is low.
  always_latch if (!clk) dout = mem[rp];
endmodule

interface fifo_if (
    input logic clk
);
  logic rst = 1'b1;
  logic push = 1'b0;
  logic pop = 1'b0;
  logic stall = 1'b0;
  logic [7:0] din = '0;
  logic [7:0] dout;
  logic full;
  logic empty;
  logic [2:0] count;
  event overflow;

  clocking cb @(posedge clk);
    default input #1step output #1;
    output push, pop, din;
    input dout, full, empty, count, stall;
  endclocking

  a_overflow :
  assert property (@(cb) disable iff (rst) !(push && full))
  else ->overflow;
endinterface

package env_pkg;
  import "DPI-C" function chandle c_model_new(input int depth);
  import "DPI-C" context function int c_model_push(
    input chandle model,
    input int value
  );
  import "DPI-C" context function int c_model_pop(
    input  chandle model,
    output int     value
  );
  import "DPI-C" function int c_model_latch(input chandle model);
  import "DPI-C" function void c_model_free(input chandle model);
  export "DPI-C" function sv_note;

  int notes[string];
  function void sv_note(input string what);
    if (notes.exists(what)) notes[what]++;
    else notes[what] = 1;
  endfunction

  typedef struct packed {
    bit       push;
    bit       pop;
    bit [7:0] data;
  } op_t;

  class driver;
    virtual fifo_if vif;
    mailbox #(op_t) ops;
    int stalled;
    function new(virtual fifo_if vif, mailbox#(op_t) ops);
      this.vif = vif;
      this.ops = ops;
    endfunction
    task run();
      op_t op;
      forever begin
        @(vif.cb);
        if (vif.cb.stall) begin
          stalled++;
          op = '0;
        end else if (!ops.try_get(op)) op = '0;
        vif.cb.push <= op.push;
        vif.cb.pop  <= op.pop;
        vif.cb.din  <= op.data;
      end
    endtask
  endclass

  class scoreboard #(
      type T = int
  );
    static int checks;
    int mismatches;
    function void compare(string what, T actual, T expected);
      checks++;
      if (actual !== expected) begin
        mismatches++;
        $display("[%0t] mismatch %s: %0d, expected %0d", $time, what, actual, expected);
      end
    endfunction
  endclass

  class monitor;
    virtual fifo_if vif;
    chandle model;
    scoreboard #(int) values;
    scoreboard #(int) levels;
    scoreboard #(bit) flags;
    int pushes;
    int pops;
    covergroup cg with function sample (bit [2:0] level, bit push, bit pop);
      option.per_instance = 1;
      cp_level: coverpoint level {bins levels[] = {[0 : 4]};}
      cp_push: coverpoint push;
      cp_pop: coverpoint pop;
      x_ops: cross cp_push, cp_pop;
    endgroup
    function new(virtual fifo_if vif);
      this.vif = vif;
      model = c_model_new(4);
      values = new();
      levels = new();
      flags = new();
      cg = new();
    endfunction
    task run();
      int expected;
      forever begin
        @(vif.cb);
        cg.sample(vif.cb.count, vif.push, vif.pop);
        // The model sees the same requests the FIFO sees at this edge and
        // decides both from the fill level before it.
        levels.compare("level", c_model_latch(model), int'(vif.cb.count));
        if (vif.push) begin
          flags.compare("push accepted", bit'(c_model_push(model, int'(vif.din))), !vif.cb.full);
          if (!vif.cb.full) pushes++;
        end
        if (vif.pop) begin
          flags.compare("pop accepted", bit'(c_model_pop(model, expected)), !vif.cb.empty);
          if (!vif.cb.empty) begin
            pops++;
            values.compare("pop data", int'(vif.cb.dout), expected);
          end
        end
      end
    endtask
  endclass
endpackage

program automatic test (
    fifo_if bus
);
  import env_pkg::*;
  int overflows;

  initial begin
    mailbox #(op_t) ops = new();
    driver drv = new(bus, ops);
    monitor mon = new(bus);
    // p = push, o = pop, b = both, n = neither.
    string script = "pppppponbooooonpbpbppppoooo";
    foreach (script[i]) begin
      op_t op;
      op.push = script[i] inside {"p", "b"};
      op.pop  = script[i] inside {"o", "b"};
      op.data = 8'(8'h40 + i);
      ops.put(op);
    end
    repeat (2) @(bus.cb);
    bus.rst = 1'b0;
    fork
      drv.run();
      mon.run();
      forever @(bus.overflow) overflows++;
    join_none
    wait (ops.num() == 0);
    repeat (4) @(bus.cb);
    $display("[%0t] pushes=%0d pops=%0d stalled=%0d overflows=%0d", $time, mon.pushes, mon.pops,
             drv.stalled, overflows);
    $display("checks: values=%0d flags=%0d mismatches=%0d model count=%0d fifo count=%0d",
             scoreboard#(int)::checks, scoreboard#(bit)::checks,
             mon.values.mismatches + mon.levels.mismatches + mon.flags.mismatches,
             c_model_latch(mon.model), bus.count);
    foreach (notes[what]) $display("note %s=%0d", what, notes[what]);
    $display("coverage level=%0.2f ops=%0.2f group=%0.2f", mon.cg.cp_level.get_inst_coverage(),
             mon.cg.x_ops.get_inst_coverage(), mon.cg.get_inst_coverage());
    c_model_free(mon.model);
    $finish;
  end
endprogram

module tb;
  logic clk = 1'b0;
  always #5 clk = ~clk;

  fifo_if bus (clk);
  fifo #(
      .DEPTH(4)
  ) dut (
      .clk  (clk),
      .rst  (bus.rst),
      .push (bus.push),
      .pop  (bus.pop),
      .din  (bus.din),
      .dout (bus.dout),
      .full (bus.full),
      .empty(bus.empty),
      .count(bus.count)
  );
  test t (bus);
endmodule
