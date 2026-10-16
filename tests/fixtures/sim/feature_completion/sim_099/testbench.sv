// SIM-099 integrated testbench: a fixed registered ALU (RTL) driven by a
// class-based testbench through a virtual interface. It composes timed
// interface and class tasks, a bounded mailbox pipeline, semaphores,
// constrained randomization with inline and switched-off constraints,
// randsequence, concurrent and immediate assertions, an embedded covergroup
// with coverage queries, a DPI-C reference model and a VPI value-change
// callback (testbench_vpi.c). Every expected line is derived in readme.md.
`timescale 1ns / 1ns

module alu (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       valid,
    input  logic [1:0] op,
    input  logic [7:0] a,
    input  logic [7:0] b,
    output logic       out_valid,
    output logic [8:0] result
);
  always_ff @(posedge clk or negedge rst_n)
    if (!rst_n) begin
      out_valid <= 1'b0;
      result <= '0;
    end else begin
      out_valid <= valid;
      if (valid)
        case (op)
          2'd0: result <= a + b;
          2'd1: result <= {1'b0, a - b};
          2'd2: result <= {1'b0, a & b};
          default: result <= {1'b0, a ^ b};
        endcase
    end
endmodule

interface alu_if (input logic clk);
  logic rst_n;
  logic valid;
  logic [1:0] op;
  logic [7:0] a;
  logic [7:0] b;
  logic out_valid;
  logic [8:0] result;

  // A timed interface task called through the virtual interface.
  task automatic reset(int cycles);
    rst_n = 1'b0;
    valid = 1'b0;
    op = '0;
    a = '0;
    b = '0;
    repeat (cycles) @(negedge clk);
    rst_n = 1'b1;
  endtask

  // Operation 3 is reserved: the testbench drives it once on purpose.
  reserved_op :
  assert property (@(posedge clk) disable iff (!rst_n) valid |-> op != 2'd3)
  else $display("[%0t] assertion reserved_op failed", $time);

  // Every accepted request is answered on the next cycle.
  handshake :
  assert property (@(posedge clk) disable iff (!rst_n) valid |=> out_valid)
  else $display("[%0t] assertion handshake failed", $time);
endinterface

package tb_pkg;
  import "DPI-C" pure function int ref_alu(input int op, input int a, input int b);

  class txn;
    rand bit [1:0] op;
    rand bit [7:0] a;
    rand bit [7:0] b;
    constraint legal {op != 2'd3; a < 8'd100; b < 8'd100;}
  endclass

  class driver;
    virtual alu_if vif;
    mailbox #(txn) requests;
    mailbox #(txn) sent;
    function new(virtual alu_if vif, mailbox#(txn) requests, mailbox#(txn) sent);
      this.vif = vif;
      this.requests = requests;
      this.sent = sent;
    endfunction
    task run();
      txn t;
      forever begin
        requests.get(t);
        @(negedge vif.clk);
        vif.valid = 1'b1;
        vif.op = t.op;
        vif.a = t.a;
        vif.b = t.b;
        sent.put(t);
        @(negedge vif.clk);
        vif.valid = 1'b0;
      end
    endtask
  endclass

  class monitor;
    virtual alu_if vif;
    mailbox #(int) results;
    function new(virtual alu_if vif, mailbox#(int) results);
      this.vif = vif;
      this.results = results;
    endfunction
    task run();
      forever begin
        @(posedge vif.clk);
        if (vif.out_valid) results.put(vif.result);
      end
    endtask
  endclass

  class scoreboard;
    mailbox #(txn) sent;
    mailbox #(int) results;
    semaphore finished;
    int expected_count;
    int checked;
    int mismatches;
    covergroup cg with function sample (bit [1:0] op, bit [7:0] a);
      cp_op: coverpoint op {
        bins add = {0};
        bins sub = {1};
        bins and_op = {2};
        bins reserved = {3};
      }
      cp_a: coverpoint a {
        bins low = {[0 : 49]};
        bins mid = {[50 : 99]};
        bins high = {[100 : 255]};
      }
    endgroup
    function new(mailbox#(txn) sent, mailbox#(int) results, int expected_count);
      this.sent = sent;
      this.results = results;
      this.expected_count = expected_count;
      finished = new(0);
      cg = new;
    endfunction
    task run();
      txn t;
      int got;
      int want;
      forever begin
        results.get(got);
        sent.get(t);
        want = ref_alu(t.op, t.a, t.b);
        cg.sample(t.op, t.a);
        checked++;
        assert (got == want)
        else mismatches++;
        $display("[%0t] check op=%0d a=%0d b=%0d dut=%0d ref=%0d", $time, t.op, t.a, t.b, got,
                 want);
        if (checked == expected_count) finished.put(1);
      end
    endtask
  endclass

  class env;
    virtual alu_if vif;
    mailbox #(txn) requests;
    mailbox #(txn) sent;
    mailbox #(int) results;
    semaphore bus_lock;
    driver drv;
    monitor mon;
    scoreboard sb;
    int n;

    function new(virtual alu_if vif);
      this.vif = vif;
      requests = new(1);
      sent = new();
      results = new();
      bus_lock = new(1);
      drv = new(vif, requests, sent);
      mon = new(vif, results);
      sb = new(sent, results, 6);
    endfunction

    // Randomize a request with every field pinned by the inline
    // constraint, then queue it; a full mailbox blocks the caller.
    task send(string who, bit [1:0] op, bit [7:0] a, bit [7:0] b);
      txn t = new;
      int ok;
      if (op == 2'd3) t.legal.constraint_mode(0);
      ok = t.randomize() with {
        op == local:: op;
        a == local:: a;
        b == local:: b;
      };
      requests.put(t);
      $display("[%0t] %s queued op=%0d a=%0d b=%0d ok=%0d", $time, who, t.op, t.a, t.b, ok);
    endtask

    task agent_a();
      bus_lock.get(1);
      $display("[%0t] A holds the bus", $time);
      n = 0;
      randsequence (main)
        main : arith logic_ops;
        arith : add sub;
        logic_ops : repeat (2) and_op;
        add : { send("A", 0, 10, 20); };
        sub : { send("A", 1, 50, 8); };
        and_op : { send("A", 2, 60 + n, 15); n++; };
      endsequence
      bus_lock.put(1);
    endtask

    task agent_b();
      txn probe = new;
      int ok;
      #1;
      bus_lock.get(1);
      $display("[%0t] B holds the bus", $time);
      // The legal constraint rejects the reserved operation...
      ok = probe.randomize() with { op == 2'd3; };
      $display("[%0t] B reserved under legal: ok=%0d", $time, ok);
      // ... and a free randomization stays inside it.
      ok = probe.randomize();
      $display("[%0t] B free: ok=%0d legal=%0d", $time, ok,
               probe.op != 2'd3 && probe.a < 100 && probe.b < 100);
      send("B", 1, 5, 9);
      send("B", 3, 85, 170);
      bus_lock.put(1);
    endtask

    task run();
      fork
        drv.run();
        mon.run();
        sb.run();
      join_none
      vif.reset(1);
      $display("[%0t] reset released", $time);
      fork
        agent_a();
        agent_b();
      join
      sb.finished.get(1);
      $display("[%0t] checked=%0d mismatches=%0d", $time, sb.checked, sb.mismatches);
      $display("coverage op=%0.2f a=%0.2f group=%0.2f", sb.cg.cp_op.get_coverage(),
               sb.cg.cp_a.get_coverage(), sb.cg.get_coverage());
      disable fork;
    endtask
  endclass
endpackage

module tb;
  import tb_pkg::*;
  logic clk = 1'b0;
  always #5 clk = ~clk;

  alu_if bus (clk);
  alu dut (
      .clk(clk),
      .rst_n(bus.rst_n),
      .valid(bus.valid),
      .op(bus.op),
      .a(bus.a),
      .b(bus.b),
      .out_valid(bus.out_valid),
      .result(bus.result)
  );

  env e;
  initial begin
    e = new(bus);
    e.run();
    $display("[%0t] done", $time);
    $finish;
  end
endmodule
