// FIN-001 G3 (F05-11): an associative array nested in a resizable container
// element (`int aa[string][int]`) is legal (SV 7.8, 12.7.3) but its elements
// have no value form: element access and foreach over it reject.
module tb;
  int aa[string][int];
  initial begin
    aa["x"][1] = 10;
    foreach (aa[k, j]) $display("%s %0d %0d", k, j, aa[k][j]);
    $finish;
  end
endmodule
