// SIM-020 llg limit: a nonblocking unpack into a whole dynamic array or
// string is rejected.
module tb;
  byte d[];
  initial begin
    {>>{d}} <= 16'h1122;
    $finish;
  end
endmodule
