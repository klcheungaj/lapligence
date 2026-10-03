// RTL-011: an `inout uwire` formal stays a frontend diagnostic (retained
// boundary; see readme). A uwire actual on an inout port is admitted.
module single(inout uwire p); assign p = 0; endmodule
module tb; wire p; single u(p); initial begin #1 $display("%b", p); $finish(0); end endmodule
