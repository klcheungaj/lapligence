// A port with a `uwire` side merges; a net with one driver stays legal and
// both names read it at once (23.3.3.6, Table 23-1 L41637).
module sink(input uwire n);
endmodule

module source(output uwire o);
  logic d = 1'b0;
  assign o = d;
endmodule

module tb;
  logic x = 1'b1;
  uwire outer;
  wire inner;
  wire driven;
  assign outer = x;
  assign inner = x;
  sink u0(.n(outer));
  sink u1(.n(inner));
  source u2(.o(driven));
  initial begin
    #1 $display("%b %b %b %b %b %b", outer, u0.n, inner, u1.n, driven, u2.o);
    x = 1'b0;
    $display("%b %b", u0.n, u1.n);
    #1 $display("%b %b", u0.n, u1.n);
    $finish(0);
  end
endmodule
