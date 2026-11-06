// SIM-025 A03: a report evaluates at the end of the slot, so a handle assigned
// after the call is what it reads. A null handle at that point stops the run as
// any access through a null handle does (SV 8.4, decision S25-D9), and the
// report prints nothing.
`timescale 1ns/1ns
module tb;
  class N;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  N h, a;
  initial begin
    a = new(7);
    $strobe("%0t first v=%0d", $time, h.v);
    h = a;
    #1 $display("%0t next", $time);
    $strobe("%0t second v=%0d", $time, h.v);
    h = null;
    #1 $display("not reached");
    $finish(0);
  end
endmodule
