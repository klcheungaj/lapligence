// A cell of an element that is a fixed unpacked array of a dynamic array,
// queue or associative array (`da[k][j]`, `q[k][j]`, `aa[s][j]`), read and
// written in place: constant and run-time indices, compound and increment
// operators, part and bit selects of a cell, string and real cells, a cell of
// a nested queue, an output actual, and `wait` and `@` on a cell.
// IEEE 1800-2009 7.4.5 (L8122): "A multidimensional array is an array of
// arrays."; 7.4.6 (L8234-8238): an out-of-range index in a fixed dimension
// reads the default and its write does nothing; 7.8.6 (L8720-8723): reading a
// missing associative key gives the default and creates no entry; 7.10.1
// (L9041-9047): "writing to Q[$+1] is legal".
module tb;
  int da[][4];
  int q[$][4];
  int aa[string][4];
  int ai[int][4];
  logic [7:0] lb[][3];
  string sa[][2];
  real ra[$][2];
  int qq[$][$][4];
  int x, j = 2, k = 1, far = 7;
  task automatic setv(output int v); v = 42; endtask
  function automatic int twice(int v); return 2 * v; endfunction
  initial begin
    da = new[2];
    da[1][2] = 7; da[k][3] = 8;
    $display("da[1][2] %0d da[k][j+1] %0d da %p", da[1][2], da[k][j+1], da);
    da[k][j]++; da[k][j] += 5;
    $display("inc %0d", da[1][2]);
    // An inner (fixed) index out of range: the read gives the default and
    // the write does nothing.
    x = da[0][far]; da[0][far] = 9;
    $display("inner oob %0d da[0] %p", x, da[0]);
    q.push_back('{1, 2, 3, 4});
    x = q[0][j]; q[0][j] = 9;
    $display("q[0][j] %0d q %p", x, q);
    q[1][0] = 4;
    $display("append size %0d q[1] %p", q.size(), q[1]);
    aa["a"][1] = 3;
    $display("aa %p num %0d", aa, aa.num());
    x = aa["zz"][1];
    $display("missing %0d num %0d", x, aa.num());
    ai[7][j] = 9;
    $display("ai %0d num %0d", ai[7][j], ai.num());
    lb = new[1];
    lb[0][j] = 8'ha5; lb[0][j][3:0] = 4'h3;
    $display("lb %h bit %b", lb[0][j], lb[0][j][5]);
    sa = new[1];
    sa[0][j-1] = "hi";
    $display("sa '%s' len %0d", sa[0][j-1], sa[0][j-1].len());
    ra.push_back('{1.5, 2.5});
    ra[0][j-1] = ra[0][j-2] * 2.0;
    $display("ra %0.1f", ra[0][1]);
    qq.push_back({}); qq[0].push_back('{1, 2, 3, 4}); qq[0][0][j] = 8;
    $display("qq %p", qq);
    setv(da[0][j]);
    x = twice(da[0][j]);
    $display("out %0d call %0d", da[0][2], x);
    fork
      begin wait (da[0][3] == 5); $display("woke at %0d", $time); end
      begin #3 da[0][3] = 5; end
    join
    fork
      begin @(da[0][j]); $display("event at %0d %0d", $time, da[0][j]); end
      begin #2 da[0][j] = 1; end
    join
    $finish;
  end
endmodule
