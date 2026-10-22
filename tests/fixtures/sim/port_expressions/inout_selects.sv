// Inout port references with part-selects and bit-selects connect the
// selected bits bidirectionally; resolution uses the inner net type.
module m(.p(a[1:0]), .q({b, a[3]}));
  inout [3:0] a;
  inout b;
  logic en;
  assign a = en ? 4'b1001 : 4'bzzzz;
  assign b = en ? 1'b0 : 1'bz;
  initial begin en = 1; #2 en = 0; end
endmodule

module mw(.p(a[1:0]));
  inout wand [3:0] a;
  assign a = 4'b1111;
endmodule

module tb;
  wire [1:0] w;
  wire [1:0] v;
  wire [1:0] y;
  logic drw;
  assign w = drw ? 2'b10 : 2'bzz;
  assign y = 2'b01;
  m c(.p(w), .q(v));
  mw d(.p(y));
  initial begin
    drw = 0;
    #1 $display("1 w=%b v=%b a=%b b=%b y=%b da=%b", w, v, c.a, c.b, y, d.a);
    #2 $display("3 w=%b v=%b a=%b b=%b", w, v, c.a, c.b);
    drw = 1;
    #1 $display("4 w=%b v=%b a=%b b=%b", w, v, c.a, c.b);
    $finish;
  end
endmodule
