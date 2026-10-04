// RTL-012: `unconnected_drive` lifetime and net-type composition
// (IEEE 1364-2001 19.9, 7.13; IEEE 1800-2009 22.9, 23.3). Expected output in
// directive_lifetime.out is hand-derived.
`include "directive_pull0.svh"
module p0(input a, input tri1 b, input wand c, input supply1 d, input var logic v,
          input [3:0] w);
  assign (weak0, weak1) c = 1'b1;
endmodule
`resetall
module plain(input a, input tri0 b, input tri1 c);
endmodule
`unconnected_drive pull1
module p1 #(parameter int W = 2) (input [W-1:0] a, input wire b);
  assign (strong0, strong1) b = 1'b0;
endmodule
`nounconnected_drive
module after(input a);
endmodule
module tb;
  p0 u0();
  plain u1();
  p1 #(3) u2[1:0]();
  for (genvar g = 0; g < 1; g++) begin : gen
    p1 u3();
  end
  after u4();
  initial begin
    #1;
    $display("p0 %v %v %v %v %b %b %v", u0.a, u0.b, u0.c, u0.d, u0.v, u0.w, u0.w[2]);
    $display("plain %v %v %v", u1.a, u1.b, u1.c);
    $display("p1 %b %v %v", u2[0].a, u2[0].a[2], u2[0].b);
    $display("p1 %b %v %v", u2[1].a, u2[1].a[0], u2[1].b);
    $display("p1 %b %v %v", gen[0].u3.a, gen[0].u3.a[1], gen[0].u3.b);
    $display("after %v", u4.a);
    $finish(0);
  end
endmodule
