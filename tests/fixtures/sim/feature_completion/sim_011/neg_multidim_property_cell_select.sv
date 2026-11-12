// SIM-011 boundary: a bit select of one element of a multidimensional class
// property rejects with its location; select the element whole.
class grid_c;
  int g[2][3];
endclass
module tb;
  grid_c o;
  initial begin
    o = new;
    o.g[1][2][3] = 1'b1;
    $display("%0d", o.g[1][2]);
  end
endmodule
