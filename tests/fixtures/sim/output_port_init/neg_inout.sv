// An inout port cannot have an initializer.
module child(inout logic o = 1'b1);
endmodule

module tb;
  wire w;
  child c(.o(w));
endmodule
