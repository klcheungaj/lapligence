// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/event_record_property.sv
// Edge and change controls on a member of an unpacked structure property
// reached through a chain of handles (SV 9.4.2). The control follows the
// object the chain names when it is armed: a store into the record of another
// object does not resume it, and a store into a different member evaluates the
// expression again without resuming.
module tb;
  timeunit 1ns; timeprecision 1ns;
  typedef struct {
    int a;
    string s;
    real x;
  } rec_t;
  class C;
    rec_t r;
  endclass
  class D;
    C c;
  endclass
  D d;
  C other;
  initial begin
    d = new;
    d.c = new;
    other = new;
    #1 d.c.r.a = 1;
    #1 d.c.r.s = "x";
    #1 other.r.a = 1;
    #1 d.c.r.a = 0;
    #1 d.c.r.a = 2;
    #1 d.c.r.x = 0.5;
    #1 d.c.r.s = "y";
    #1 d.c.r.a = 3;
    #1 d.c = other;
    #1 d.c.r.a = 5;
    #1 $display("%0t end", $time);
    $finish(0);
  end
  initial begin
    @(posedge d.c.r.a) $display("%0t posedge a=%0d", $time, d.c.r.a);
    @(negedge d.c.r.a) $display("%0t negedge a=%0d", $time, d.c.r.a);
    @(d.c.r.x) $display("%0t x=%0.1f", $time, d.c.r.x);
    @(d.c.r.a or d.c.r.s) $display("%0t a or s: a=%0d s=%s", $time, d.c.r.a, d.c.r.s);
    @(posedge d.c.r.a) $display("%0t posedge a=%0d", $time, d.c.r.a);
    @(d.c.r.a) $display("%0t a=%0d", $time, d.c.r.a);
  end
endmodule
