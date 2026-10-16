// Procedural concurrent assertions (IEEE 1800-2009 16.15.6): each module
// covers one rule; readme.md derives every expected line.
`timescale 1ns/1ns

// 16.15.6 a)-c): the clock comes from the procedure's event control before
// default clocking; `iff` stays part of it; a clock term read in the body
// stops the inference.
module inferred;
  logic clk = 0, clk2 = 0;
  logic a = 0, en = 0, q1, q2;
  always #5 clk = ~clk;
  always #3 clk2 = ~clk2;
  default clocking @(posedge clk2); endclocking
  always @(posedge clk) begin
    q1 <= a;
    i1: assert property (a) $display("[%0t] i1 pass", $time); else $display("[%0t] i1 fail", $time);
  end
  always @(posedge clk iff en) begin
    i2: assert property (a) $display("[%0t] i2 pass", $time); else $display("[%0t] i2 fail", $time);
  end
  always @(posedge clk) begin
    q2 <= clk;
    i3: assert property (a) $display("[%0t] i3 pass", $time); else $display("[%0t] i3 fail", $time);
  end
  initial begin
    #12 a = 1; en = 1;
    #10 en = 0;
  end
endmodule

// Instances are queued only where execution reaches the statement, once per
// execution, and a matured instance waits for its own leading clock.
module queued;
  logic clk = 0, clk2 = 0;
  logic a = 1, b = 0, en = 0;
  int i;
  always #5 clk = ~clk;
  always #10 clk2 = ~clk2;
  always @(posedge clk) begin
    if (en) begin
      q1: assert property (@(posedge clk) b) $display("[%0t] q1 pass", $time); else $display("[%0t] q1 fail", $time);
      for (i = 0; i < 2; i++)
        q2: assert property (@(posedge clk) a) $display("[%0t] q2 pass", $time);
    end
  end
  always @(posedge clk) begin
    q3: assert property (@(posedge clk2) a) $display("[%0t] q3 pass", $time);
  end
  initial begin
    #12 en = 1;
    #20 en = 0;
  end
endmodule

// 16.15.6.2: resuming from an event control flushes the queue; a delay
// does not.
module flushed;
  logic clk = 0;
  logic a = 0, b = 0;
  always #5 clk = ~clk;
  default clocking @(posedge clk); endclocking
  always @(a or b) begin : b1
    f2: assert property (a == b) $display("[%0t] f2 pass", $time); else $display("[%0t] f2 fail", $time);
    #1;
    f3: assert property (a == b) $display("[%0t] f3 pass", $time); else $display("[%0t] f3 fail", $time);
  end
  initial begin
    #12 a = 1;
    #20 b = 1;
    #10 a = 0;
  end
  initial #43 b <= 0;
endmodule

// 16.15.6.4: disabling an assertion clears its pending instances; disabling
// the outermost scope of the procedure clears its whole queue.
module disabled;
  logic clk = 0;
  logic bad_val = 0, bad_val_ok = 0, c = 0, clear = 0;
  always #5 clk = ~clk;
  default clocking @(posedge clk); endclocking
  always @(bad_val or bad_val_ok) begin : b1
    d1: assert property (bad_val) $display("[%0t] d1 pass", $time); else $display("[%0t] d1 fail", $time);
    if (bad_val_ok) begin
      disable d1;
    end
  end
  always @(c) begin : b2
    d2: assert property (c) $display("[%0t] d2 pass", $time); else $display("[%0t] d2 fail", $time);
  end
  always @(clear) begin : b3
    disable b2;
  end
  initial #42 clear <= 1;
  initial begin
    #12 bad_val = 1;
    #10 bad_val_ok = 1;
    #10 c = 1;
    #10 c = 0;
  end
endmodule

// The sequence, property and cover engines begin one attempt per matured
// instance.
module engines;
  logic clk = 0;
  logic a = 0, en = 0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    if (en) begin
      e1: assert property (a ##1 !a) $display("[%0t] e1 pass", $time); else $display("[%0t] e1 fail", $time);
      e2: assert property (s_eventually a) $display("[%0t] e2 pass", $time); else $display("[%0t] e2 fail", $time);
      e3: assert property (a |=> !a) $display("[%0t] e3 pass", $time); else $display("[%0t] e3 fail", $time);
      e4: cover property (a) $display("[%0t] e4 covered", $time);
    end
  end
  initial begin
    #12 en = 1; a = 1;
    #10 en = 0; a = 0;
  end
endmodule

// Processes woken after the Observed region of the clock's time step: the
// clocking block event and program code in the Reactive region.
module late;
  logic clk = 0;
  logic a = 0;
  always #5 clk = ~clk;
  clocking cb @(posedge clk); endclocking
  always @(cb) begin
    l1: assert property (@(posedge clk) a) $display("[%0t] l1 pass", $time); else $display("[%0t] l1 fail", $time);
  end
  initial #12 a = 1;
  program automatic prg;
    initial begin
      repeat (2) begin
        @(posedge clk);
        l2: assert property (@(posedge clk) a) $display("[%0t] l2 pass", $time); else $display("[%0t] l2 fail", $time);
      end
      // Ending every program calls $finish (24.3); stay alive past tb's.
      #40;
    end
  endprogram
  prg u_prg();
endmodule

// An initial procedure queues one instance; fork branches are processes
// with their own queues.
module once;
  logic clk = 0;
  logic a = 0;
  always #5 clk = ~clk;
  initial begin
    #7;
    o1: assert property (@(posedge clk) a) $display("[%0t] o1 pass", $time); else $display("[%0t] o1 fail", $time);
  end
  initial begin
    fork
      begin #11; o2: assert property (@(posedge clk) a) $display("[%0t] o2 pass", $time); end
      begin #21; o3: assert property (@(posedge clk) !a) $display("[%0t] o3 pass", $time); else $display("[%0t] o3 fail", $time); end
    join_none
  end
  initial #12 a = 1;
endmodule

// Property and sequence local variables belong to the attempt, not to the
// procedure, and are not captured.
module locals;
  logic clk = 0;
  logic [3:0] d = 0;
  always #5 clk = ~clk;
  sequence s_inc(local input logic [3:0] v);
    (1, v = v + 1) ##1 d == v;
  endsequence
  property p_loc;
    logic [3:0] x;
    (1, x = d) ##1 d == x + 1;
  endproperty
  always @(posedge clk) begin
    v1: assert property (@(posedge clk) s_inc(d)) $display("[%0t] v1 pass", $time); else $display("[%0t] v1 fail", $time);
    v2: assert property (@(posedge clk) p_loc) $display("[%0t] v2 pass", $time); else $display("[%0t] v2 fail", $time);
    d <= d + 1;
  end
endmodule

module tb;
  inferred u_inferred();
  queued u_queued();
  flushed u_flushed();
  disabled u_disabled();
  engines u_engines();
  late u_late();
  once u_once();
  locals u_locals();
  initial #46 $finish;
endmodule
