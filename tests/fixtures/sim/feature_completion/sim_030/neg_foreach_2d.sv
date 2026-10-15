// foreach over more than one dimension is not supported.
class c_c;
  rand bit [3:0] m [2][3];
  constraint c { foreach (m[i, j]) m[i][j] == i + j; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
