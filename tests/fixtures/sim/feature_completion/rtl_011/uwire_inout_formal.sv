// RTL-011/RTL-105: an `inout uwire` formal with its only driver inside the
// child (IEEE 1800-2009 6.6.2, 23.3.3.6-23.3.3.7). RTL-011 retained it as a
// frontend rejection; RTL-105 admits it.
module single(inout uwire p); assign p = 0; endmodule
module tb; wire p; single u(p); initial begin #1 $display("%b", p); $finish(0); end endmodule
