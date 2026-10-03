// SV2009 7.4.6, 11.5, 23.2.2.3: selected refs across 64-bit limbs.
module child(ref logic [64:0] x);
 initial begin
  #1;
  x[64 -: 3] = 3'b101;
  x[-1 +: 3] = 3'b111;
  $display("wide %b %b %b", x[64 -: 3], x[65 -: 3], x[64 +: 3]);
  x[65 -: 3] = 3'b011;
 end
endmodule
module tb;
 logic [95:0] value = 0;
 child c(value[80:16]);
 initial begin #2; $display("root %h", value); $finish(0); end
endmodule
