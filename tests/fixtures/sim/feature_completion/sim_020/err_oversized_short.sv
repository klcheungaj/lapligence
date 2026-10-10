// SIM-020 A02/A03: an oversized fixed unpack target needs every bit of its
// width from the source (SV 11.4.14.3); it is left unmodified on error.
module tb;
  localparam int N = 70000;
  logic [15:0] big [N];
  logic [15:0] q [$];
  initial begin
    q = {16'h0001, 16'h0002};
    $display("before %0d", q.size());
    {>>{big}} = q;
    $display("after %h", big[0]);
    $finish;
  end
endmodule
