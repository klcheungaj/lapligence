module sub(input a, output y);
  assign y = a;
endmodule
module tb;
  reg r;
  wire a;
  sub u(.a(a), .y(r));
endmodule
