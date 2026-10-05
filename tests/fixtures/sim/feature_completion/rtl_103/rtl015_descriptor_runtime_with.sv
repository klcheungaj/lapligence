// IEEE 1800-2009 11.4.14.4: formerly the RTL-015 negative
// `neg_descriptor_runtime_with`. Runtime `with` ranges of an oversized
// descriptor array stream as runtime selections, without flattening.
module tb;
  localparam int N = 70000;
  logic [15:0] src [N];
  logic [15:0] rot [N];
  int i;
  initial begin
    foreach (src[k]) src[k] = 16'(k);
    i = 4;
    rot = {>>{src with [i : N-1], src with [0 : i-1]}};
    $display("%h %h %h %h", rot[0], rot[N-5], rot[N-4], rot[N-1]);
    $finish(0);
  end
endmodule
