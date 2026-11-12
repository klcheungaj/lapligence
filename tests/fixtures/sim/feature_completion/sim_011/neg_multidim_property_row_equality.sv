// SIM-011 boundary: equality of a row of a multidimensional class property
// rejects with its location; compare elements or copy the row first.
class grid_c;
  int g[2][3];
endclass
module tb;
  grid_c o;
  int r[3];
  initial begin
    o = new;
    if (o.g[0] == r) $display("equal");
  end
endmodule
