// A module-body specparam is declared before it is referenced (the frontend
// reports it).
module m(input a, output y);
  wire #(S) w = a;
  specparam S = 2;
  assign y = w;
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
