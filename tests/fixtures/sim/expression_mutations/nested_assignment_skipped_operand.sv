// A nested assignment to a string runs only where its full expression runs
// it first (decision EO-D1); in an operand that `?:` may skip it is rejected
// with its location.
module tb;
  string s, s2;
  bit c;
  initial begin
    c = 1;
    s2 = c ? (s = "a") : "b";
    $display("%s %s", s, s2);
    $finish;
  end
endmodule
