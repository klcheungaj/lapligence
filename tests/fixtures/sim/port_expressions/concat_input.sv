// Input port expressions that concatenate nets and variables; the actual is
// split most significant part first.
module mn(.p({a, b}));
  input a;
  input [1:0] b;
endmodule

module mv(.p({a, b}));
  input logic a;
  input var logic [1:0] b;
endmodule

module tb;
  logic [2:0] d;
  mn m1(.p(d));
  mv m2(.p(d ^ 3'b111));
  initial begin
    d = 3'b101;
    #1 $display("1 net a=%b b=%b var a=%b b=%b", m1.a, m1.b, m2.a, m2.b);
    d = 3'b010;
    #1 $display("2 net a=%b b=%b var a=%b b=%b", m1.a, m1.b, m2.a, m2.b);
    $finish;
  end
endmodule
