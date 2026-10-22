// An inout port expression concatenating nets is a bidirectional connection:
// both sides resolve the same bits.
module m(.p({a, b}));
  inout a;
  inout [1:0] b;
  logic en;
  assign a = en ? 1'b1 : 1'bz;
  assign b = en ? 2'b00 : 2'bzz;
  initial begin en = 1; #2 en = 0; end
endmodule

module tb;
  wire [2:0] w;
  logic dr;
  assign w = dr ? 3'b011 : 3'bzzz;
  m m1(.p(w));
  initial begin
    dr = 0;
    #1 $display("1 w=%b a=%b b=%b", w, m1.a, m1.b);
    #2 $display("3 w=%b a=%b b=%b", w, m1.a, m1.b);
    dr = 1;
    #1 $display("4 w=%b a=%b b=%b", w, m1.a, m1.b);
    $finish;
  end
endmodule
