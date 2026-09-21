// llg-test-fixture: SYN-003 assignment-pattern lvalue shape rejection.
// LRM: IEEE 1800-2009 10.9.
module tb;
  typedef logic [7:0] U [0:1];
  logic [7:0] a, b, c;
  U values;
  initial begin
    '{a, b, c} = values;
    $finish;
  end
endmodule
