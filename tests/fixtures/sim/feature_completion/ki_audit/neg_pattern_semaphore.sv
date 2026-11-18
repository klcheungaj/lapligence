// `%p` of a semaphore handle held directly in the argument: a located limit
// (docs/known_issues.md); 21.2.1.7 leaves the text to the implementation.
module tb;
  semaphore s = new;
  initial begin
    $display("%p", s);
    $finish;
  end
endmodule
