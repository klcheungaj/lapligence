// Indexed part-selects of fixed arrays as by-value actuals.
module tb;
  int arr[4] = '{1, 2, 3, 4};
  int dsc[3:0] = '{4, 3, 2, 1};
  int off[5:8] = '{1, 2, 3, 4};
  logic [3:0] nib[2:5] = '{4'h1, 4'h2, 4'h3, 4'h4};
  int m[2][3] = '{'{1, 2, 3}, '{4, 5, 6}};
  int k = 1, j = 6, h = 3, r;
  function automatic int f(int x[2]);
    $display("x %0d %0d", x[0], x[1]);
    return x[0] + x[1];
  endfunction
  function automatic int g(logic [3:0] x[2]);
    $display("g %h %h", x[0], x[1]);
    return x[0] + x[1];
  endfunction
  initial begin
    r = f(arr[k+:2]);
    r = f(arr[2-:2]);
    r = f(arr[k+1-:2]);
    r = f(dsc[k+:2]);
    r = f(dsc[h-:2]);
    r = f(off[j+:2]);
    r = f(off[j-:2]);
    r = g(nib[h+:2]);
    r = g(nib[h-:2]);
    r = f(m[1][k+:2]);
    $display("%0d", f(arr[k+:2]));
    k = 3;
    r = f(arr[k+:2]);
    r = g(nib[5+:2]);
    $finish(0);
  end
endmodule
