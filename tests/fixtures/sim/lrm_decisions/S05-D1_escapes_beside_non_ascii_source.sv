// S05-D1: non-ASCII source text leaves string values alone: café here is
// UTF-8, and an octal escape below is still one byte of any value.
// 5.9 L4103: "A string literal is a sequence of characters enclosed by double
// quotes." Café.
module tb;
  string s = "\377A";

  initial begin
    $display("%0d %h %0d", s.len(), s[0], s[1]);
    $finish;
  end
endmodule
