// One element of a multidimensional class property (7.4.5, 8.4): through a
// handle and inside a method, with constant, loop and property indices, for
// reads and writes; an index outside its dimension reads the default value
// (7.4.6). A copy of the object has its own elements.
class grid_c;
  bit [3:0] g[2][3];
  bit [7:0] d[1:2][3:1];
  bit [1:0] x = 1, y = 2;
  int n = 1;
  function void fill();
    foreach (g[i, j]) g[i][j] = i * 4 + j;
    d[2][1] = 7;
  endfunction
endclass

module tb;
  initial begin
    static grid_c o = new;
    grid_c p;
    int a = 1, b = 2, total;
    bit [1:0] c = 1, e = 2;
    o.fill();
    $display("%0d %0d %0d", o.g[1][2], o.g[0][1], o.d[2][1]);
    $display("%0d %0d %0d %0d", o.g[a][b], o.g[c][e], o.g[o.x][o.y], o.g[o.n][o.n]);
    o.g[1][0] = 9;
    $display("%p", o.g);
    foreach (o.g[i]) foreach (o.g[i][j]) total += o.g[i][j];
    $display("%0d", total);
    p = new o;
    p.g[1][1]++;
    $display("%0d %0d %0d", o.g[1][1], p.g[1][1], o.g[a + 1][0]);
    $finish(0);
  end
endmodule
