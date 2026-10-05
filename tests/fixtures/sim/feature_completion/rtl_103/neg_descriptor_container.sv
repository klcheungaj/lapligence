// A resizable container operand of an oversized descriptor stream belongs to
// the dynamic-stream owner (SIM-020); it is rejected rather than flattened.
module tb;
  localparam int N = 70000;
  logic [15:0] src [N];
  logic [15:0] rot [N];
  logic [15:0] q [$];
  initial begin
    q.push_back(16'h1);
    rot = {>>{q, src with [0 : N-2]}};
    $finish(0);
  end
endmodule
