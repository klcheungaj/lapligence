// 6.6.2 L4794: "It shall be an error to connect any bit of a uwire net to
// more than one driver". The port merges, so the formal's driver is a second
// driver of the actual.
module child(input wire n);
  logic d = 1'bz;
  assign n = d;
endmodule

module tb;
  logic x = 1'b1;
  uwire a;
  assign a = x;
  child u0(.n(a));
endmodule
