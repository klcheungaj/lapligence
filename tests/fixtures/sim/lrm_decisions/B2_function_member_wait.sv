// B2: wait and @ on a function that reads an object member through a
// class-handle argument.
//
// IEEE 1800-2009 9.4.2 (L11837-11840): "Changing the value of object data
// members, aggregate elements, or the size of a dynamically sized array
// referenced by a method or function shall cause the event expression to be
// reevaluated. An implementation can cause the event expression to be
// reevaluated when changing the value or size even if the members are not
// referenced by the method or function."
// 9.4.3 (L12032-12033): "The wait statement shall evaluate a condition; and,
// if it is false, the procedural statements following the wait statement
// shall remain blocked until that condition becomes true before continuing."
//
// Decision: writing h.x re-evaluates get_x(h); the event control resumes when
// the function's value changes and the wait when its condition becomes true.
`timescale 1ns / 1ns
module tb;
  class C;
    int x;
  endclass

  C h;

  function automatic int get_x(C c);
    return c.x;
  endfunction

  initial begin
    h = new;
    fork
      begin
        @(get_x(h));
        $display("%0t: @(get_x(h)) resumed, value %0d", $time, get_x(h));
      end
      begin
        wait (get_x(h) == 2);
        $display("%0t: wait (get_x(h) == 2) resumed", $time);
      end
    join_none
    #1 h.x = 0;
    #1 h.x = 1;
    #1 h.x = 2;
    #1 $display("%0t: done", $time);
    $finish;
  end
endmodule
