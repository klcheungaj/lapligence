// SIM-011 boundary: a row of a multidimensional class property is read into
// and written from array variables, patterns and rows, and printed; passing it
// to a fixed-array formal rejects with its location.
class grid_c;
  int g[2][3];
endclass
module tb;
  grid_c o;
  function automatic int sum(int v[3]);
    return v[0] + v[1] + v[2];
  endfunction
  initial begin
    o = new;
    $display("%0d", sum(o.g[1]));
  end
endmodule
