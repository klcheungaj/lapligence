// B1: wait and @ on a string class property.
//
// IEEE 1800-2009 9.4.2 (L11822-11824): "A variable used with the event
// control can be any one of the integral data types (see 6.11.1) or string.
// The variable can be either a simple variable or a ref argument (variable
// passed by reference); it can be a member of an array, associative array, or
// object (class instance) of the aforementioned types."
// 9.4.3 (L12032-12033): "The wait statement shall evaluate a condition; and,
// if it is false, the procedural statements following the wait statement
// shall remain blocked until that condition becomes true before continuing."
//
// Decision: a write that changes a string property wakes both forms; an equal
// store is not a change and wakes neither.
`timescale 1ns / 1ns
module tb;
  class C;
    string s;
  endclass

  C h;

  initial begin
    h = new;
    h.s = "a";
    fork
      begin
        wait (h.s == "go");
        $display("%0t: wait (h.s == \"go\") resumed", $time);
      end
      begin
        @(h.s);
        $display("%0t: @(h.s) resumed, h.s = %s", $time, h.s);
      end
    join_none
    #1 h.s = "a";
    #1 h.s = "b";
    #1 h.s = "go";
    #1 $display("%0t: done", $time);
    $finish;
  end
endmodule
