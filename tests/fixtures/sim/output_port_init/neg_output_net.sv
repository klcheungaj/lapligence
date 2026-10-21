// An output net port cannot have an initializer.
module child(output wire o = 1'b1);
endmodule

module tb;
  wire w;
  child c(.o(w));
endmodule
