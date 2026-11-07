// 6.21: an element of a class property takes no nonblocking write.
module tb;
  class C;
    int fix[3];
  endclass
  C h = new;
  initial h.fix[1] <= 8;
endmodule
