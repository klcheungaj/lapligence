// Ports whose merge would change the result keep their continuous
// assignment (decision A21-D2): a child driving its input net, a second
// driver of an output actual, internal net types that could dominate, a
// net delay of the formal, a type conversion.
module coerced(input wire n);
  assign n = 1'b0;
endmodule

module drv(output wire o, input logic d);
  assign o = d;
endmodule

module typed(input tri0 t, input wand a, input wire [1:0] w);
endmodule

module delayed(n);
  input n;
  wire #2 n;
endmodule

module tb;
  logic x;
  wire c, m, z, dl;
  wor wo;
  wire [3:0] wide;
  assign c = x;
  assign m = 1'b0;
  assign wo = x;
  assign wo = 1'b0;
  assign dl = x;
  assign wide = {4{x}};
  coerced u0(.n(c));
  drv u1(.o(m), .d(x));
  typed u2(.t(z), .a(wo), .w(wide));
  delayed u3(.n(dl));
  initial begin
    x = 1'b1;
    #1 $display("coerced: c=%b u0.n=%b", c, u0.n);
    $display("second driver: m=%b u1.o=%b", m, u1.o);
    $display("types: z=%b u2.t=%b wo=%b u2.a=%b u2.w=%b", z, u2.t, wo, u2.a, u2.w);
    $display("delay: dl=%b u3.n=%b", dl, u3.n);
    #2 $display("delay later: dl=%b u3.n=%b", dl, u3.n);
    $finish(0);
  end
endmodule
