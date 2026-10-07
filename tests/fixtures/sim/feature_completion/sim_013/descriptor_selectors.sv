// SIM-013 A01 boundary: a 65,537-cell array (descriptor storage) observed
// through a runtime selector, by an event control and a level wait.
`timescale 1ns / 1ns
module tb;
  int big[65537];
  int i = 0;
  string l_big = "";
  int t_wait = -1;

  initial #0 forever begin @(big[i]); l_big = {l_big, $sformatf(" %0t", $time)}; end
  initial begin
    #0 wait (big[i] == 3);
    t_wait = $time;
  end

  initial begin
    #1 big[65536] = 1;
    #1 big[0] = 2;
    #1 i = 65536;
    #1 big[0] = 3;
    #1 big[65536] = 3;
    #1;
    $display("big[i]:%s", l_big);
    $display("wait: %0d", t_wait);
    $finish(0);
  end
endmodule
