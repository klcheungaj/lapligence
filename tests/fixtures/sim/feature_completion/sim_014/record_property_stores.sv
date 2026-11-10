// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/record_property_stores.sv
// Every way of storing into an unpacked structure property with a string
// member reaches a waiter on its members (SV 9.4.2): member and whole-record
// assignments, output and `ref` actuals (the record, an integral member and a
// string member; a `ref` store is seen when it is made, inside the task),
// increments, compound assignments, `$sscanf` and `$sformat` targets, and a
// method of the object.
typedef struct { int a; string s; real x; } rec_t;
class H;
  rec_t r;
  task set(int v); r.a = v; endtask
endclass
module tb;
  timeunit 1ns; timeprecision 1ns;
  H h = new;
  rec_t t;
  task automatic out(output rec_t o); o.a = 7; o.s = "o"; endtask
  task automatic outi(output int o); o = 9; endtask
  task automatic rf(ref rec_t o); #1 o.a = 8; #1 o.s = "rf"; endtask
  task automatic rfi(ref int o); #1 o = 10; endtask
  task automatic rfs(ref string o); #1 o = "rs"; endtask
  initial forever begin
    @(h.r.a or h.r.s or h.r.x);
    $display("%0t a=%0d s=%s x=%0.1f", $time, h.r.a, h.r.s, h.r.x);
  end
  initial begin
    #1 h.r.a = 1;
    #1 t.a = 3; t.s = "t"; h.r = t;
    #1 out(h.r);
    #1 outi(h.r.a);
    #1 h.r.a++;
    #1 h.r.a += 2;
    #1 h.r.s = {h.r.s, "x"};
    #1 void'($sscanf("42 zz", "%d %s", h.r.a, h.r.s));
    #1 $sformat(h.r.s, "f%0d", 3);
    #1 h.set(11);
    #1 h.r.x = 2.5;
    #1 rf(h.r);
    #1 rfi(h.r.a);
    #1 rfs(h.r.s);
    #1 h.r = '{5, "p", 1.5};
    #1 h.r = '{5, "p", 1.5};
    #1 $display("%0t end", $time);
    $finish(0);
  end
endmodule
