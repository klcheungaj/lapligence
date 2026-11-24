// Elaboration system tasks (IEEE 1800-2009 20.10): $info and $warning print
// once per call left after generate expansion and the simulation runs.
`timescale 1ns/1ps
interface ifc #(int N = 3) ();
  logic a = N[0];
  $info("N=%0d", N);
endinterface
checker ck(logic c);
  $info("checker");
endchecker
program prg;
  $warning("program");
endprogram
module sub #(parameter int W = 8) ();
  $info;
  $info();
  $warning("W=%0d %m", W);
  if (W == 8) begin : g8
    $info("g8");
  end else begin : other
    $info("not 8");
  end
  for (genvar I = 0; I < 2; I++) begin : g
    $info("I=%0d", I);
    if (I == 1) begin : inner
      $warning("last");
    end
  end
  case (W)
    8: begin : c8 $info("case 8"); end
    default: begin : cdef $info("case default"); end
  endcase
  initial $display("%m %0d", W);
endmodule
module bnd;
  $info("bound");
endmodule
module unused;
  $info("never instantiated");
  $fatal(1, "never instantiated");
endmodule
module tb;
  $info("real %g, string %s", 1.5, "abc");
  if (0) begin : untaken
    $error("untaken");
    $fatal(0, "untaken");
  end
  sub #(8) u();
  sub #(4) v[0:1]();
  ifc #(5) fi();
  prg p();
  ck c0(fi.a);
  bind sub bnd b();
  initial #1 $display("run %b", fi.a);
  initial #2 $finish;
endmodule
