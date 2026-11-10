// Decision S07-D18: a member of an unpacked structure that holds a string,
// kept in a class property or in a static subroutine variable, is waited on
// through the structure. A store into any member evaluates the waiter's
// expression again; the waiter resumes when the value of its own expression
// changed, so `@(h.r.a)` does not resume for a store into `h.r.s`.
//
// IEEE 1800-2009 9.4.2 (SystemVerilog-1800-2009.txt L11807): "An implicit
// event shall be detected on any change in the value of the expression."
// L11822-11824: "The variable can be either a simple variable or a ref
// argument (variable passed by reference); it can be a member of an array,
// associative array, or object (class instance) of the aforementioned types."
//
// The value is compared at the check of decision T1-D11, for every member: a
// change undone before the writing process suspends is no change, also for
// the string member (a string variable of its own wakes on each change, see
// T1-D11_aggregates_at_check). The lines from 21 to 24 are that choice: a
// simulator that wakes at each write prints one line at each of those times.
//
// Expected output: one line per store from 11 to 14, none from 21 to 24.
`timescale 1ns / 1ns
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;
  class H;
    rec_t r;
  endclass
  H h = new;
  task keep();
    static rec_t r;
  endtask
  always @(h.r.a) $display("%0t h.r.a=%0d", $time, h.r.a);
  always @(h.r.s) $display("%0t h.r.s=%s", $time, h.r.s);
  always @(keep.r.a) $display("%0t keep.r.a=%0d", $time, keep.r.a);
  always @(keep.r.s) $display("%0t keep.r.s=%s", $time, keep.r.s);
  initial begin
    #10 $display("-- 10 one member at a time");
    #1 h.r.a = 1;
    #1 h.r.s = "s";
    #1 keep.r.a = 2;
    #1 keep.r.s = "t";
    #6 $display("-- 20 changes undone before the check");
    #1 h.r.a = 5; h.r.a = 1;
    #1 h.r.s = "x"; h.r.s = "s";
    #1 keep.r.a = 5; keep.r.a = 2;
    #1 keep.r.s = "x"; keep.r.s = "t";
    #6 $display("-- 30 end");
    $finish(0);
  end
endmodule
