// Equality of rows of multidimensional native arrays is legal (SV 11.4.5)
// but not lowered: code generation rejects it explicitly.
module tb;
  string a[2][2];
  string b[2][2];
  initial begin
    if (a[0] == b[1]) $display("same");
    $finish;
  end
endmodule
