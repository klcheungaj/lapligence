// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/static_record_nba_waits.sv
// Nonblocking stores into a static subroutine record with a string member
// reach its waiters when they commit (SV 10.4.2, 9.4.2): a member, a string
// member and the whole record. Two nonblocking stores in one time step that
// leave the member as it was are no change (decision T1-D11), and a store
// into the member `x` evaluates a waiter on `a` again without resuming it.
// An `always @*` that reads a member follows the record too (SV 9.4.2.2).
module tb;
  timeunit 1ns; timeprecision 1ns;
  typedef struct { int a; string s; real x; } rec_t;
  int wakes;
  int y;
  task keep();
    static rec_t r;
  endtask
  always @* y = keep.r.a + 1;
  initial forever begin
    @(keep.r.a or keep.r.s);
    wakes++;
    $display("%0t a=%0d s=%s x=%0.1f", $time, keep.r.a, keep.r.s, keep.r.x);
  end
  initial begin
    #1 keep.r.a <= 6;
    #1 keep.r.s <= "n";
    #1 keep.r <= '{7, "w", 2.5};
    #1 keep.r.a <= 8;
    keep.r.a <= 7;
    #1 keep.r.x <= 3.5;
    #1 keep.r.a <= #2 9;
    #1 $display("%0t a=%0d y=%0d (not yet)", $time, keep.r.a, y);
    #2 $display("%0t end wakes=%0d y=%0d", $time, wakes, y);
    $finish(0);
  end
endmodule
