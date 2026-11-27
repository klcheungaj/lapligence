// Remaining items of unpacked array concatenations: an item of a run-time
// size into a fixed target of two dimensions.
module tb;
  int rows[$][2];
  int r2[2] = '{3, 4};
  int m[2][2];
  initial begin
    rows.push_back('{1, 2});
    m = {rows, r2};
    $display("%p", m);
  end
endmodule
