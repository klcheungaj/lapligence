module tb;
  int w[$][2];
  int a2[int][2];
  int qq[$][$];
  int ad[int][];
  int sq[string][$];
  int s3[string][int][$];
  initial begin
    w.push_back('{1, 2});
    w.push_back('{3, 4});
    foreach (w[, j]) $display("j=%0d", j);
    foreach (w[i,]) $display("i=%0d", i);
    a2[3] = '{5, 6};
    foreach (a2[, j]) $display("j=%0d", j);
    foreach (a2[k, j]) $display("%0d %0d", k, j);
    ad[5] = '{1, 2};
    ad[7] = '{3};
    foreach (ad[k, j]) $display("%0d %0d %0d", k, j, ad[k][j]);
    sq["a"] = '{1};
    sq["b"] = '{4, 5};
    foreach (sq[k, j]) $display("%s %0d %0d", k, j, sq[k][j]);
    foreach (sq[k]) $display("%s %0d", k, sq[k].size());
    qq.push_back('{9});
    foreach (qq[i, j]) $display("%0d %0d %0d", i, j, qq[i][j]);
    $finish;
  end
endmodule
