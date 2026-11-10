module tb;
  int qq[$][$];
  initial begin
    qq.push_back({});
    qq[0].push_back(1);
    $strobe("%p", qq[0]);
    #1 $finish;
  end
endmodule
