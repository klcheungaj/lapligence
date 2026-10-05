`define CONN .a
module sub(input a);
endmodule
module tb;
  wire a;
  sub u(`CONN);
endmodule
