// PATHPULSE$ pulse limits are ADV-008: rejected with their location.
module m(input a, output y);
  assign y = a;
  specify
    specparam PATHPULSE$a$y = (1, 2);
    (a => y) = 3;
  endspecify
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
