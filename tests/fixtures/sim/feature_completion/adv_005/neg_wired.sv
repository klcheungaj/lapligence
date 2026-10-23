// ADV-005: wired-logic nets in a switch network are not simulated.
module tb;
  wand w;
  wire x;
  assign w = 1'b1;
  tran t1(w, x);
  initial $finish;
endmodule
