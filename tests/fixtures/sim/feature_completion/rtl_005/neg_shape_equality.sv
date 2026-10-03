// SV2009 6.22.2, 11.4.5: unpacked arrays compare only with equal shapes.
module tb;
  logic [3:0] a [0:2];
  logic [3:0] b [0:3];
  initial begin
    $display("%b", a == b);
    $finish(0);
  end
endmodule
