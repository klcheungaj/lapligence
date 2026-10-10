// SIM-020 A02: a runtime-sized source shorter than its static unpack
// targets is a run-time error (SV 11.4.14.3) raised before any target write.
module tb;
  int a, b, c;
  int d[];
  initial begin
    d = new[2];
    d[0] = 7;
    a = 5;
    $display("before %0d", a);
    {>>{a, b, c}} = {>>{d}};
    $display("after %0d %0d %0d", a, b, c);
    $finish;
  end
endmodule
