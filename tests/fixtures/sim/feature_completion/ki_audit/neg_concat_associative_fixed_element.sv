// Remaining items of unpacked array concatenations: an item of a run-time
// size into a fixed associative-array element.
module tb;
  int q[$] = '{1, 2};
  int fa[int][3];
  initial begin
    fa[1] = {q, 0};
    $display("%p", fa[1]);
  end
endmodule
