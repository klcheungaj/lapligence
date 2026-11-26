// Nonblocking assignments under a clocking event. It happens in the Observed
// region (14.13), where no update can be queued.
`timescale 1ns/1ns
interface bus;
  logic clk = 1'b0;
  clocking cb @(posedge clk);
  endclocking
endinterface

class item;
endclass

module tb;
  bus b();
  virtual bus vb = b;
  logic clk = 1'b0;
  clocking cb @(posedge clk);
  endclocking
  int v, w, x, y, n;
  int elements[2];
  real r;
  string s, names[2];
  item made = new, h;
  process p;

  task automatic tick;
    #1 clk = 1'b1;
    b.clk = 1'b1;
    #1 clk = 1'b0;
    b.clk = 1'b0;
  endtask

  initial begin
    #1;
    v <= @(cb) 1;
    w <= @(b.cb) 2;
    x <= @(vb.cb) 3;
    y <= repeat (2) @(cb) 4;
    n <= @(cb iff clk) 5;
    elements[1] <= @(cb) 6;
    r <= @(cb) 1.5;
    s <= @(cb) "seven";
    names[1] <= @(cb) "eight";
    h <= @(cb) made;
    #1 $display("before %0d %0d %0d", v, w, x);
    tick();
    $display("first  %0d %0d %0d %0d %0d %0d %.1f %s %s %0d", v, w, x, y, n,
             elements[1], r, s, names[1], h != null);
    tick();
    $display("second %0d", y);

    // It follows the updates of the turn that makes the clocking event.
    v <= @(cb) 1;
    #1 clk = 1'b1;
    v <= 9;
    #1 $display("order  %0d", v);
    clk = 1'b0;

    // A clocking event is a named event: the update ends with its issuer.
    fork
      begin
        p = process::self();
        v <= @(cb) 2;
        x <= @(vb.cb) 2;
        s <= @(cb) "two";
      end
    join_none
    #1 p.kill();
    tick();
    $display("killed %0d %0d %s", v, x, s);
    $finish(0);
  end
endmodule
