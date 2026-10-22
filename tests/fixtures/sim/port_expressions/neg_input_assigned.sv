// A variable declared as an input port cannot be assigned, even when its port
// expression selects only some of its bits.
module m(.p(v[1:0]));
  input var logic [3:0] v;
  initial v[0] = 1'b1;
endmodule

module tb;
  m c(.p(2'b10));
endmodule
