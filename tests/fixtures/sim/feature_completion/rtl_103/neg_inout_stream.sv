// IEEE 1800-2009 11.4.14: a streaming concatenation is an assignment target
// or source, or a bit-stream cast operand; an inout actual is both read and
// written and is rejected by the frontend.
module tb;
  logic [7:0] arr [0:7];
  int n;
  task automatic swap(inout logic [15:0] v);
    v = {v[7:0], v[15:8]};
  endtask
  initial begin
    n = 1;
    swap({>>{arr with [n +: 2]}});
    $finish(0);
  end
endmodule
