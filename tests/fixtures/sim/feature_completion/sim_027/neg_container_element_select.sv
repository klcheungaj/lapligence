// Known limitation (docs/known_issues.md): an element of an unpacked array
// inside a dynamic, queue or associative element is reached only through
// memory file tasks, whole-element copies and `%p`; selecting one cell in an
// expression is a located error.
module tb;
  int da [][4];
  int x;
  initial begin
    da = new[2];
    x = da[1][2];
    $display("%0d", x);
  end
endmodule
