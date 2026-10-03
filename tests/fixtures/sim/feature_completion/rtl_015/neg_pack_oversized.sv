// IEEE 1800-2009 11.4.14.3: a stream wider than its fixed-size target is an
// error (`int j = {>>{ a, b, c }};`).
module tb;
  int a, b, c;
  int j;
  initial begin
    j = {>>{a, b, c}};
    $finish;
  end
endmodule
