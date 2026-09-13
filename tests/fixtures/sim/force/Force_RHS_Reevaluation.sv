// IEEE 1800-2009 §10.6.2 and IEEE 1364-2001 §9.3.2: a force expression
// remains active and follows changes in its right-hand-side dependencies.
`timescale 1ns/1ps
module tb;
  reg q, d;
  initial begin
    q = 0;
    d = 1;
    force q = d;
    #1 $display("CHECK: initial=%b", q);
    d = 0;
    #1 $display("CHECK: follows=%b", q);
    release q;
    $finish(0);
  end
endmodule
