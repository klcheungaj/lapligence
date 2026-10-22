// An inout port expression cannot connect to a variable.
module m(.p({a, b}));
  inout a, b;
endmodule

module tb;
  logic [1:0] v;
  m c(.p(v));
endmodule
