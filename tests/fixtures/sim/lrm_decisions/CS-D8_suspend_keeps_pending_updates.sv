// Decision CS-D8: suspend() of a process leaves the updates and nonblocking
// triggers it has issued alone. Each commits or triggers at its own time,
// while the process is suspended too, like those of a process that runs on:
// nonblocking assignments (also with a delay and to class handles and static
// class properties) and triggers with and without a delay or event control.
// A resume does not repeat or move them.
//
// IEEE 1800-2009 9.7 (SystemVerilog-1800-2009.txt L12639):
//   "The suspend() function allows a process to suspend either its own execution or that of another process."
// L12644-12645:
//   "The resume() function restarts a previously suspended process. Calling resume on a process that was sus-
//   pended while blocked on another condition shall resensitize the process to the event expression or to wait for"
// L12631-12632 (kill(), unlike suspend(), names the subprocesses):
//   "The kill() function terminates the given process and all its subprocesses, that is, processes spawned using
//   fork statements by the process being killed."
// An industry-standard simulator, when the process is suspended in the time
// slot of the updates, never commits the class handle and static property
// updates (h=0 sp=0 to the end), and it holds back the triggers with a delay
// while the process is suspended, then lets them wait the rest of their delay
// (e0 at 20, e5 at 25 and at 54); it leaves real child processes running.
// llg keeps every update and trigger at its time.
`timescale 1ns/1ns
class obj;
  static int sp;
  int v;
  function new(int value);
    v = value;
  endfunction
endclass
module tb;
  logic clk = 0;
  event e5, ee, e1, e0;
  int x;
  obj h;
  process p;
  always @(e5) $display("[%0t] e5", $time);
  always @(ee) $display("[%0t] ee", $time);
  always @(e1) $display("[%0t] e1", $time);
  always @(e0) $display("[%0t] e0", $time);
  initial begin
    #12;
    forever begin
      clk = 1'b1;
      #2 clk = 1'b0;
      #4;
    end
  end
  task automatic issue(int v);
    obj o = new(v);
    ->> #5 e5;
    ->> @(posedge clk) ee;
    ->> e1;
    ->> #0 e0;
    x <= v;
    x <= #5 v + 1;
    h <= o;
    obj::sp <= v;
  endtask
  task show(string at);
    $display("[%0t] %s x=%0d h=%0d sp=%0d %s", $time, at, x, h == null ? 0 : h.v, obj::sp,
             p.status().name());
  endtask
  initial begin
    // Suspended in the time slot of the updates.
    #10;
    fork
      begin
        p = process::self();
        issue(1);
        #100;
      end
    join_none
    #0 p.suspend();
    #1 show("suspended");
    #5 show("suspended");
    #4 p.resume();
    #1 show("resumed");
    #5 show("resumed");
    p.kill();
    // Suspended one unit after the updates.
    #14;
    fork
      begin
        p = process::self();
        issue(3);
        #100;
      end
    join_none
    #1 p.suspend();
    show("suspended");
    #5 show("suspended");
    #4 p.resume();
    #1 show("resumed");
    #5 $finish;
  end
endmodule
