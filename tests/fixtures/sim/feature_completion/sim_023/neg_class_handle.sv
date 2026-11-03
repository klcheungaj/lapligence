// Decision S23-D5: a class-handle variable is not a force target.
class C;
endclass
module tb;
  C c;
  initial force c = null;
endmodule
