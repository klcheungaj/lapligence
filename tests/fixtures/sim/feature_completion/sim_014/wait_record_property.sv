// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/wait_record_property.sv
// A member of an unpacked structure property that holds a string is waited on
// through the record: a store into any member evaluates the waiter's
// expression again, and the waiter resumes when its value changed (SV 9.4.2).
// The record of M keeps a queue in each element of a member array.
typedef struct { int a; string s; } rec_t;
typedef struct { int q[$]; int e2; } el_t;
class H;
  rec_t r;
endclass
class M;
  el_t e[3];
endclass
module tb;
  timeunit 1ns; timeprecision 1ns;
  H h = new;
  M m = new;
  int k = 1;
  initial begin
    #5 h.r.a = 1;
    #5 h.r.s = "s";
    #5 h.r.a = 2;
    #5 m.e[k].q.push_back(1);
    #5 $display("%0t end", $time);
    $finish(0);
  end
  initial begin
    wait (h.r.a == 1);
    $display("%0t wait h.r.a == 1 woke", $time);
    @(h.r.a);
    $display("%0t @(h.r.a) woke with %0d", $time, h.r.a);
    wait (m.e[k].q.size() == 1);
    $display("%0t wait m.e[k].q.size() woke", $time);
  end
  initial begin
    @(h.r.s);
    $display("%0t @(h.r.s) woke with '%s'", $time, h.r.s);
  end
endmodule
