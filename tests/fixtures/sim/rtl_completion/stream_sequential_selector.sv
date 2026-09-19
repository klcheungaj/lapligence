module tb;
  int count;
  logic [7:0] lanes[0:1];
  initial begin
    count = 1;
    lanes[0] = 0;
    lanes[1] = 0;
    {>>{count, lanes with [0 +: count]}} = 48'h000000021122;
    $display("count=%0d lanes=%h,%h", count, lanes[0], lanes[1]);
    $finish(0);
  end
endmodule
