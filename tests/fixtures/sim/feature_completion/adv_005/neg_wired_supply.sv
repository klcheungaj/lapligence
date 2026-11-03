// FIN-001 G3 (F08-11): a switch network that joins a wired-logic net and a
// supply (or tri0/tri1) net is legal but not simulated: how the two net
// types combine across a switch is not settled.
module tb;
  wor o;
  wire p;
  supply1 vdd;
  assign o = 1'b0;
  tran t1(o, p);
  tran t2(p, vdd);
  initial $finish;
endmodule
