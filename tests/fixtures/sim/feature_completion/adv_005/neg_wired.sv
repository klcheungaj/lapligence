// FIN-001 G3 (F08-11): a switch network that joins a wired-AND and a
// wired-OR net is legal but not simulated: the two wired functions would
// need opposite orders in one solve.
module tb;
  wand w;
  wor o;
  wire x;
  assign w = 1'b1;
  assign o = 1'b0;
  tran t1(w, x);
  tran t2(x, o);
  initial $finish;
endmodule
