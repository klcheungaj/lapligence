// ADV-013 A02: the maximal singly clocked subsequence before a clock change
// is `a ##1 b[*0:1]`, which admits only nonempty matches, so the multiclocked
// sequence is legal (16.14.1 L25903-25904) although its last element
// `b[*0:1]` alone could match empty. With b low, only the `b[*0]` branch
// (`a ##0 1`, 16.9.2.1) matches; once b is high both covers match.
module tb;
  logic c1 = 0, c2 = 0, a = 1, b = 0, d = 1;
  int n1, n2;
  always #5 c1 = ~c1;
  always #7 c2 = ~c2;
  cover property (@(posedge c1) a ##1 b[*0:1] ##1 @(posedge c2) d) n1++;
  cover property (@(posedge c1) a ##1 b ##1 @(posedge c2) d) n2++;
  initial begin
    #52 b = 1;
    #50 $display("n1=%0d n2=%0d", n1, n2);
    $finish;
  end
endmodule
