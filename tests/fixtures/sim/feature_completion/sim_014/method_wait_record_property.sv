// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/method_wait_record_property.sv
// A method's waits on members of its own unpacked structure property, by its
// bare name and through `this`, and a derived class's method waiting on the
// inherited record while the store goes through a handle of the base class
// (SV 9.4.2).
module tb;
  timeunit 1ns; timeprecision 1ns;
  typedef struct {
    int a;
    string s;
  } rec_t;
  class C;
    rec_t r;
    task t();
      wait (r.s == "go");
      $display("%0t own wait woke", $time);
      @(this.r.a);
      $display("%0t own @ woke a=%0d", $time, r.a);
    endtask
    task set(int v);
      r.a = v;
    endtask
  endclass
  class E extends C;
    int extra;
    task u();
      wait (r.a == 7);
      $display("%0t derived wait woke", $time);
    endtask
  endclass
  C h;
  E e;
  C b;
  initial begin
    h = new;
    e = new;
    b = e;
    fork
      h.t();
      e.u();
      begin
        #1 h.r.s = "go";
        #1 h.set(4);
        #1 b.r.a = 7;
      end
    join
    $display("%0t end", $time);
    $finish(0);
  end
endmodule
