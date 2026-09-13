`timescale 1ns/1ps
module tb;
  logic [7:0] value;
  logic signed [7:0] signed_value;
  logic [3:0] xz_value;

  initial begin
    value = 8'h2a;
    signed_value = -8'sd3;
    xz_value = 4'b1x0z;

    $displayb(value);
    $displayo(value);
    $displayh(value);
    $writeb(value);
    $writeo(value);
    $writeh(value);
    $displayh("%d", signed_value);
    $displayb(xz_value);
    $displayh(xz_value);

    #1 begin
      value <= 8'h3c;
      $strobeb(value);
      $strobeo(value);
      $strobeh(value);
    end
    #1 $monitorb(value);
    #1 value <= 8'h15;
    #1 $monitoro(value);
    #1 $monitorh(value);
    #1 $finish(0);
  end
endmodule
