// IEEE 1800-2009 §10.6.2: selected net force overlays preserve the current
// resolved drivers, including replacement, disjoint masks, X conflicts and Z
// release behavior.
`timescale 1ns/1ps
module tb;
  reg [3:0] drive_a, drive_b;
  wire [3:0] n;
  assign n = drive_a;
  assign n = drive_b;

  initial begin
    drive_a = 4'b1010;
    drive_b = 4'bzzzz;
    #1 force n[3:2] = 2'b01;
    force n[3:2] = 2'b00;
    force n[1:0] = 2'b10;
    drive_a = 4'b1100;
    #1 $display("forced=%b", n);
    release n[3:2];
    $display("upper_release=%b", n);
    release n[1:0];
    $display("all_release=%b", n);
    drive_b = 4'b0011;
    #1 $display("conflict=%b", n);
    drive_a = 4'bzzzz;
    #1 $display("z_release=%b", n);
    $finish(0);
  end
endmodule
