// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/static_record_waits.sv
// A static subroutine record with a string member is waited on through the
// record, like an unpacked structure property (SV 9.4.2): member and
// whole-record stores, `ref` actuals and stores from the task itself reach
// an event control and a `wait` that name its members.
module tb;
  timeunit 1ns; timeprecision 1ns;
  typedef struct { int a; string s; real x; } rec_t;
  int wait_a = -1, wait_s = -1;
  task automatic rfi(ref int o); #1 o = 10; endtask
  task automatic rf(ref rec_t o); #1 o.s = "rf"; endtask
  task keep(input int v, input string t);
    static rec_t r;
    r.a = v;
    r.s = t;
  endtask
  initial forever begin
    @(keep.r.a or keep.r.s or keep.r.x);
    $display("%0t a=%0d s=%s x=%0.1f", $time, keep.r.a, keep.r.s, keep.r.x);
  end
  initial begin
    wait (keep.r.a == 3);
    wait_a = $time;
    wait (keep.r.s == "rf");
    wait_s = $time;
  end
  initial begin
    #1 keep(1, "one");
    #1 keep(1, "two");
    #1 keep.r.a = 3;
    #1 keep.r.x = 0.5;
    #1 rfi(keep.r.a);
    #1 rf(keep.r);
    #1 keep.r = '{5, "p", 1.5};
    #1 $display("%0t end wait_a=%0d wait_s=%0d", $time, wait_a, wait_s);
    $finish(0);
  end
endmodule
