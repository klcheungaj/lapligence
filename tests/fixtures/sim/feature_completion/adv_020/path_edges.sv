`timescale 1ns/1ns
module cell_t(input a, b, c, d, output y, z);
  assign y = a ^ b; assign z = c ^ d;
  specify (posedge a, b *> (y +: a)) = (5, 6); if (a) (negedge c, d *> (z -: c)) = 7; endspecify
endmodule
module tb;
  reg a, b, c, d; wire yu, zu, yv, zv, yw, zw;
  cell_t u(a, b, c, d, yu, zu), v(a, b, c, d, yv, zv), w(a, b, c, d, yw, zw);
  initial begin
    $sdf_annotate("path_edges.sdf", tb);
    #1 $edges("tb.u"); $edges("tb.v"); $edges("tb.w");
    $monitor("%0t a%b b%b c%b d%b | %b%b %b%b %b%b", $time, a, b, c, d, yu, zu, yv, zv, yw, zw);
    a = 0; b = 0; c = 0; d = 0;
    #200 a = 1; #200 b = 1; #200 b = 0; #200 c = 1; #200 d = 1; #200 d = 0; #200 c = 0; #200 a = 0; #200 $finish;
  end
endmodule
