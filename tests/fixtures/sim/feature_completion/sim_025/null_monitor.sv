// SIM-025 A03: a monitor reads its arguments when it evaluates, so a path that
// reaches a null handle then stops the run (SV 8.4, decision S25-D9).
`timescale 1ns/1ns
module tb;
  class N;
    int v;
  endclass
  N h;
  initial begin
    h = new;
    $monitor("%0t v=%0d", $time, h.v);
    #1 h.v = 1;
    #1 h = null;
    #1 $display("not reached");
    $finish(0);
  end
endmodule
