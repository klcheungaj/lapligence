// IEEE 1800-2009 11.4.14.4: constant `with` ranges of oversized fixed arrays
// stream like the equivalent slices, so a whole-array stream of two ranges
// stays a descriptor stream instead of one packed value. Expected values are
// independent derivations of the rotated and reversed element order.
module tb;
  localparam int N = 70000;
  logic [15:0] src [N];
  logic [15:0] rot [N];
  logic [15:0] down [N-1:0];
  logic [15:0] back [N];

  initial begin
    src[0] = 16'h0A00;
    src[3] = 16'h0A03;
    src[4] = 16'h0A04;
    src[N-1] = 16'h0AFF;
    rot = {>>{src with [4 : N-1], src with [0 : 3]}};
    $display("rot %h %h %h %h", rot[0], rot[N-5], rot[N-4], rot[N-1]);
    down = {>>{src with [1 : N-1], src with [0]}};
    $display("down %h %h %h %h", down[N-1], down[N-3], down[1], down[0]);
    back = {<<16{down with [N-1 : 1], down with [0 +: 1]}};
    $display("back %h %h %h %h", back[0], back[1], back[N-3], back[N-1]);
    $finish(0);
  end
endmodule
