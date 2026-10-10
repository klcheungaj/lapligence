// SIM-020 A03: an associative array is not a streaming unpack target.
module tb;
  byte ab[int];
  initial begin
    {>>{ab}} = 16'h0102;
    $finish;
  end
endmodule
