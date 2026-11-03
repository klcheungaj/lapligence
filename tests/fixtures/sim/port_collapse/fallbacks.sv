// Ports that keep their continuous assignment where a merge would drop
// something (decisions A21-D2, A21-D4): a net delay of the formal, a width
// conversion, a variable actual of a net formal the child drives, and a
// selected actual of such a formal.
module coerced(input wire n);
  assign n = 1'b0;
endmodule

module narrow(input wire [1:0] w);
endmodule

module delayed(n);
  input n;
  wire #2 n;
endmodule

module tb;
  logic x, vr;
  wire dl;
  wire [3:0] wide, v;
  assign dl = x;
  assign wide = {4{x}};
  assign v = {4{x}};
  narrow u2(.w(wide));
  delayed u3(.n(dl));
  coerced u4(.n(vr));
  coerced u5(.n(v[1]));
  initial begin
    x = 1'b1;
    vr = 1'b1;
    #1 $display("width: wide=%b u2.w=%b", wide, u2.w);
    $display("variable actual: vr=%b u4.n=%b", vr, u4.n);
    $display("selected actual: v=%b u5.n=%b", v, u5.n);
    $display("delay: dl=%b u3.n=%b", dl, u3.n);
    #2 $display("delay later: dl=%b u3.n=%b", dl, u3.n);
    $finish(0);
  end
endmodule
