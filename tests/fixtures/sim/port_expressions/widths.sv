// Width differences between an output port expression and its actual: a
// narrower actual takes the low bits, a wider net actual leaves its upper
// bits undriven (z).
module m(.p({a, b}));
  output [1:0] a;
  output b;
  assign a = 2'b11;
  assign b = 1'b0;
endmodule

module tb;
  wire [1:0] narrow;
  wire [4:0] wide;
  m c1(.p(narrow));
  m c2(.p(wide));
  initial #1 begin
    $display("narrow=%b wide=%b", narrow, wide);
    $finish;
  end
endmodule
