// A constant select past a string member array of a leaf-form record (below
// 16 cells): 7.4.6 reads the default and drops the write; the leaf form names
// one object per cell, so it is a located limit (docs/known_issues.md).
module tb;
  typedef struct { int x; string s[8]; } rec_t;
  rec_t r;
  initial begin
    r.s[20] = "x";
    $display("%0d", r.x);
    $finish;
  end
endmodule
