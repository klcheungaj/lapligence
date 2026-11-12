// SIM-027 boundary: a whole unpacked-array element copies to or from a whole
// fixed-array variable; a row or slice of a larger array (`m2[1] = da[1]`)
// rejects with its location.
module tb;
  int da[][4];
  int m2[2][4];
  initial begin
    da = new[2];
    m2[1] = da[1];
    $display("%p", m2);
  end
endmodule
