// Decision T1-D2: a wait or event control on the comparison of event
// variables or event formals is a compile error. A comparison naming a class
// event property is served: its rebinding wakes the wait.
//
// IEEE 1800-2009 9.4.3 (SystemVerilog-1800-2009.txt L12032-12033): "The wait
// statement shall evaluate a condition; and, if it is false, the procedural
// statements following the wait statement shall remain blocked until that
// condition becomes true". The text does not say that a handle comparison may
// not appear in the condition; a variable assignment that wakes nothing would
// block forever. Some simulators also reject the class property form.
module tb;
  event k, g;
  initial begin
    fork
      begin
        wait (k == g);
        $display("woke at %0t", $time);
      end
      #1 k = g;
    join
    $finish;
  end
endmodule
