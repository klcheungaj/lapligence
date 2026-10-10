// SIM-020 A03: elements of dynamically sized variables shall not be written
// with nonblocking assignments (SV 6.21).
module tb;
  byte d[];
  initial begin
    d = new[2];
    {>>{d[0], d[1]}} <= 16'h0102;
    $finish;
  end
endmodule
