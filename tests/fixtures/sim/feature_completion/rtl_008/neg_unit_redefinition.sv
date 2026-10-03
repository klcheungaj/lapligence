// SV2009 3.12.1, 3.13: a name is declared once in a compilation-unit scope.
int unit_k = 1;
int unit_k = 2;
module tb;
  initial begin
    $display("%0d", unit_k);
    $finish;
  end
endmodule
