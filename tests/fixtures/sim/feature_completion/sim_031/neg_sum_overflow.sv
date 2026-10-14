// Resource overflow: two weights of 1048575 bits (the widest legal packed
// value) have an exact sum of 1048576 bits, which exceeds the packed limit.
module tb;
  logic [1048574:0] wa = 1;
  logic [1048574:0] wb = 1;
  int x;
  initial begin
    randcase
      wa : x = 1;
      wb : x = 2;
    endcase
    $finish;
  end
endmodule
