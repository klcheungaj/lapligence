// An implicit port whose expression is not a simple identifier has no name,
// so it cannot be connected by name.
module m(a[1:0]);
  input [3:0] a;
endmodule

module tb;
  m c(.a(2'b01));
endmodule
