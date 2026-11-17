// ADV-019 callback kinds (FIN-001 V3 family (e)): callback_kinds.c places
// cbValueChange records on the chandle variables h and h2 and prints each
// call; see readme.md.
`timescale 1ns / 1ns
module tb;
  import "DPI-C" function chandle cbk_make(input int n);
  chandle h, h2;
  initial begin
    $cbk(0);
    #1 h = cbk_make(5);
    #1 h = cbk_make(5);
    #1 h = null;
    #1 begin
      h = cbk_make(7);
      h = null;
    end
    #1 begin
      h = cbk_make(8);
      h = cbk_make(9);
    end
    #1 h2 = h;
    #1 $cbk(1);
    #1 h = cbk_make(10);
    #1 $cbk(2);
    $display("done %0d", h == h2);
    $finish;
  end
endmodule
