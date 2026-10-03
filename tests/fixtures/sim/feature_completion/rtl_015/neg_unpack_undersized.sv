// IEEE 1800-2009 11.4.14.3: unpacking more bits than the source provides is an
// error (`{>>{ a, b, c }} = 23'b1;`).
module tb;
  int a, b, c;
  initial begin
    {>>{a, b, c}} = 23'b1;
    $finish;
  end
endmodule
