`timescale 1ns/1ps
module leaf #(parameter string NAME = "leaf");
  initial #1 $show(NAME);
endmodule
module tb;
  parameter string S = "str";
  parameter string E = "";
  localparam string L = {S, "-", "x"};
  leaf a ();
  leaf #(.NAME("other name")) b ();
  initial begin : blk
    localparam string B = "block";
    $show(S, E, L, B, "literal");
    $show(tb.a.NAME, blk.B);
  end
  task automatic t;
    localparam string T = "task";
    $show(T);
  endtask
  initial #2 t;
  initial #3 $display("%s %s", S, L);
endmodule
