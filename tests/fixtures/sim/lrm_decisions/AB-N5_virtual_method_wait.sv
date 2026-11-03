// AB-N5: a virtual method call in a wait condition is rejected.
//
// IEEE 1800-2009 9.4.2 (L11833-11835): "Nonvirtual methods of an object and
// built-in methods or system functions for an aggregate type are allowed in
// event control expressions as long as the type of the return value is
// singular and the method is defined as a function, not a task."
// 9.4.3 (L12032-12033): "The wait statement shall evaluate a condition; and,
// if it is false, the procedural statements following the wait statement
// shall remain blocked until that condition becomes true before continuing."
//
// Decision (industry-standard behaviour; 9.4.3 states no rule of its own for
// method calls in a wait condition): the 9.4.2 restriction to nonvirtual
// methods applies to wait conditions as well, so the design is rejected at
// compile time. Expected result: it prints nothing (the .out file is empty).
// A simulator that accepts it would print "woke 1".
module tb;
  class C;
    int x;
    virtual function int get();
      return x;
    endfunction
  endclass
  C h = new;
  initial begin
    wait (h.get() == 1);
    $display("woke %0d", h.x);
  end
  initial #1 h.x = 1;
endmodule
