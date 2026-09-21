// llg-test-fixture: SYN-003 assignment-pattern lvalue width rejection.
// LRM: IEEE 1800-2009 10.9.
module tb;
  typedef logic [7:0] U [0:1];
  logic [3:0] a;
  logic [7:0] b;
  U values;
  initial begin
    '{a, b} = values;
    $finish;
  end
endmodule
