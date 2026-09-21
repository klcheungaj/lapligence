// llg-test-fixture: SYN-003 constant assignment-pattern lvalue rejection.
// LRM: IEEE 1800-2009 10.9.
module tb;
  typedef logic [7:0] U [0:1];
  logic [7:0] a;
  U values;
  initial begin
    '{a, 8'h00} = values;
    $finish;
  end
endmodule
