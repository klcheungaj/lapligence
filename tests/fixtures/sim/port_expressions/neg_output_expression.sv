// An output port expression must connect to an lvalue.
module m(.p({a, b}));
  output a, b;
  assign a = 1;
  assign b = 0;
endmodule

module tb;
  wire [1:0] w;
  m c(.p(w + 1));
endmodule
