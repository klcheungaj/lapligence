// A row of a multidimensional class property (SV 7.4.2, 8.4) is an unpacked
// array operand: printed with `%p`, read into and written from an array
// variable, a row of one, a pattern or another row, also inside a method and
// for a three-dimensional property; a whole property takes an array variable.
// IEEE 1800-2009 7.4.5 (L8122) and 7.6 (L8436-8441); 7.4.6 (L8234-8238):
// a row selected by an invalid index reads the default of each cell and its
// write does nothing.
class grid_c;
  int g[2][3];
  logic [7:0] h[1:0][0:2][2];
  function void put(int k, int v[3]);
    g[k] = v;
  endfunction
endclass

module tb;
  grid_c o;
  int r[3];
  int m2[2][3];
  int d[];
  logic [7:0] p[2];
  logic [7:0] q[0:2][2];
  int i = 1, bad = 5;
  initial begin
    o = new;
    o.g[1][2] = 5; o.g[1][0] = 3;
    $display("row %p", o.g[1]);
    r = o.g[1];
    $display("r %p", r);
    r[1] = 6;
    o.g[0] = r;
    $display("g %p", o.g);
    o.g[0] = '{7, 8, 9};
    $display("g %p", o.g);
    o.g[i] = o.g[0];
    d = o.g[i];
    $display("g %p d %p", o.g, d);
    m2[0] = o.g[1];
    o.g[0] = m2[1];
    $display("m2 %p g %p", m2, o.g);
    r = '{4, 4, 4};
    r = o.g[bad];
    $display("bad read %p", r);
    r = '{4, 4, 4};
    o.g[bad] = r;
    $display("bad write %p", o.g);
    o.put(0, r);
    $display("put %p", o.g);
    o.g = m2;
    $display("whole %p", o.g);
    o.h[0][1] = '{8'h11, 8'h22};
    p = o.h[0][1];
    q = o.h[0];
    q[2][0] = 8'h33;
    o.h[1] = q;
    $display("p %h %h q20 %h h102 %h h120 %h", p[0], p[1], q[2][0], o.h[1][0][1], o.h[1][2][0]);
    $finish;
  end
endmodule
