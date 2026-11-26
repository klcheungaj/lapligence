// S09-D11: when and whether a nonblocking assignment with an event control
// happens.
// 9.4.5 (SystemVerilog-1800-2009.txt L12123): "An intra-assignment delay or
// event control shall delay the assignment of the new value to the left-hand
// side"; no clause says when in a time step or relates it to the process that
// issued it.
// Decision: a control that names an event, or reads a class property (also a
// static one), an element of a dynamic array, queue or associative array or a
// member of a virtual interface, is waited for like by a process of the
// issuer: its update is queued after the turn that triggers the event, where
// several triggers count once, and until then it ends with the issuer like a
// process that the issuer has forked (S09-D8). Any other control is watched
// for the variables and nets it reads, and its update stays pending when the
// issuer ends. Wait fork waits for neither.
`timescale 1ns/1ns
interface bus;
  logic clk = 0;
endinterface

package pkg;
  int state;
endpackage

module other;
  logic s = 0;
  event e;
endmodule

module tb;
  class ticker;
    int x;
    static int count;
  endclass
  bus b();
  virtual bus vb = b;
  other o();
  ticker t = new, made = new, h[2];
  event ev, ev2, eva[2];
  logic s = 0, g = 1;
  real rl;
  int dyn[], qu[$], as[int];
  int v, calls;
  int bound[14], free[8];
  real r;
  process p;

  function automatic int same(int value);
    calls++;
    return value;
  endfunction

  task automatic issue;
    bound[0] <= @(ev) 1;
    bound[1] <= @(eva[1]) 1;
    bound[2] <= @(o.e) 1;
    bound[3] <= @(ev iff g) 1;
    bound[4] <= repeat (2) @(ev) 1;
    bound[5] <= @(s or ev) 1;
    bound[6] <= @(t.x) 1;
    bound[7] <= @(ticker::count) 1;
    bound[8] <= @(dyn[1]) 1;
    bound[9] <= @(qu[0]) 1;
    bound[10] <= @(as[3]) 1;
    bound[11] <= @(posedge vb.clk) 1;
    bound[12] <= repeat (2) @(vb.clk) 1;
    bound[13] <= @(posedge s iff vb.clk) 1;
    r <= @(ev or ev2) 1.5;
    h[1] <= @(ev) made;
    free[0] <= @(posedge s) 1;
    free[1] <= @(posedge b.clk) 1;
    free[2] <= @(same(s)) 1;
    free[3] <= @(posedge s iff g) 1;
    free[4] <= repeat (2) @(s) 1;
    free[5] <= @(o.s) 1;
    free[6] <= @(pkg::state) 1;
    free[7] <= @(rl) 1;
  endtask

  task issuer;
    issue();
    #100;
  endtask

  task automatic show(string how);
    #1 b.clk = 1'b1;
    s = 1'b1; o.s = 1'b1; pkg::state++; rl++;
    t.x++; ticker::count++; dyn[1]++; qu[0]++; as[3]++;
    ->ev; ->eva[1]; ->o.e;
    #1 ->ev;
    s = 1'b0; b.clk = 1'b0; o.s = 1'b0;
    #1 $write("%-14s bound=", how);
    foreach (bound[i]) $write("%0d", bound[i]);
    $write(" r=%.1f h=%0d free=", r, h[1] != null);
    foreach (free[i]) $write("%0d", free[i]);
    $display;
    foreach (bound[i]) bound[i] = 0;
    foreach (free[i]) free[i] = 0;
    r = 0; h[1] = null;
    #1;
  endtask

  initial begin
    dyn = new[2]; qu = '{0}; as[3] = 0;
    #1;
    fork issue(); join
    show("returns");
    fork begin p = process::self(); issuer; end join_none
    #1 p.kill();
    show("kill");
    fork begin p = process::self(); issue(); end join_none
    #1 p.kill();
    show("finished, kill");
    fork issuer; join_none
    #1 disable fork;
    show("disable fork");
    fork
      begin : blk issuer; end
      #1 disable blk;
    join
    show("block");
    fork
      issuer;
      #1 disable issuer;
    join
    show("task");
    v <= @(ev) 5;
    wait fork;
    $write("wait fork      ");
    #1 ->ev;
    #1 $display("%0d", v);

    // The update of a named event's control follows those of its turn.
    v <= @(ev) 5;
    #1 ->ev;
    v <= 7;
    #1 $display("event, update  %0d", v);
    v <= @(ev) 5;
    #1 v <= 7;
    ->ev;
    #1 $display("update, event  %0d", v);
    v <= @(ev) 5;
    ->ev;
    v <= 8;
    #1 $display("issuing turn   %0d", v);
    v <= @(ev) 5;
    #1 ->> ev;
    v <= 9;
    #1 $display("->>            %0d", v);
    v <= @(ev) 5;
    v <= @(ev) 6;
    #1 ->ev;
    #1 $display("two controls   %0d", v);
    v = 0;
    v <= @(ev) 5;
    #1 ->ev;
    #0 $write("at #0          %0d", v);
    #1 $display(" then %0d", v);
    // Triggers in one turn count once.
    v = 0;
    v <= repeat (2) @(ev) 5;
    #1 ->ev;
    ->ev;
    #1 $write("same turn      %0d", v);
    ->ev;
    #1 $display(" then %0d", v);
    v = 0;
    v <= repeat (2) @(ev) 5;
    #1 ->ev;
    #0 ->ev;
    #1 $display("next turn      %0d", v);
    v = 0; g = 1'b0;
    v <= @(ev iff g) 5;
    #1 ->ev;
    #1 g = 1'b1;
    $write("condition      %0d", v);
    ->ev;
    #1 $display(" then %0d", v);
    // Once the event has been triggered the update no longer needs its issuer.
    v = 0;
    fork begin p = process::self(); v <= @(ev) 5; #100; end join_none
    #1 ->ev;
    #0 p.kill();
    #5 $display("after event    %0d", v);
    $finish(0);
  end
endmodule
