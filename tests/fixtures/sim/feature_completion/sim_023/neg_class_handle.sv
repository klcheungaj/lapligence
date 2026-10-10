// Legal (a class handle variable is singular) but not supported by llg.
class C;
endclass
module tb;
  C c;
  initial force c = null;
endmodule
