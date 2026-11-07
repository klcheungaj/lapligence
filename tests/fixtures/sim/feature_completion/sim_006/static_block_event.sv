module tb;
  initial begin
    static logic bs = 1;
    static int cnt = 0;
    fork
      begin @(bs); cnt++; $display("woke %b %0d", bs, cnt); end
      begin wait (cnt == 1); $display("cnt"); end
      #5 bs = 0;
    join
    fork
      begin @(posedge bs); $display("pos %b", bs); end
      #5 bs = 1;
    join
    $finish;
  end
endmodule
