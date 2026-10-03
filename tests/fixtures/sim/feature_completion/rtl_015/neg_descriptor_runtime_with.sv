// IEEE 1800-2009 11.4.14.4: a runtime `with` range gives an oversized
// descriptor stream a runtime extent, which descriptor transport cannot
// represent without flattening; it is rejected.
module tb;
  localparam int N = 70000;
  logic [15:0] src [N];
  logic [15:0] rot [N];
  int i;
  initial begin
    i = 4;
    rot = {>>{src with [i : N-1], src with [0 : i-1]}};
    $finish;
  end
endmodule
