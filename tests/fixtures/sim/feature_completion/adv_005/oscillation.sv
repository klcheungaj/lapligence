// ADV-005: a network that switches its own control off and on in zero time
// never settles; the run stops with the zero-delay loop error instead of
// hanging (each solve itself always terminates).
`timescale 1ns/1ns
module tb;
  wire a, b, c;
  reg ra, rc;
  assign (strong0, strong1) a = ra;
  assign (weak0, weak1) c = rc;
  tranif1 t1(a, b, c);
  tran t2(b, c);
  initial begin
    ra = 1; rc = 1; #1 $display("1 %v %v %v", a, b, c);
    ra = 0; #1 $display("2 %v %v %v", a, b, c);
    $finish;
  end
endmodule
