// IEEE 1800-2009 12.7.3, 7.12.3-7.12.4: foreach over mixed packed/unpacked
// dimensions, omitted slots, singleton/negative/descending bounds, formal
// declared bounds, and reductions whose with maps capture nested iterators
// and automatic locals.
module tb;
  logic [3:0][1:0] m [-1:1][2:0];
  bit [7:0] s [0:0];
  int r [3:-2];

  function automatic int formal_sum(input int a [2:0], int k);
    int t = 0;
    foreach (a[i]) t += a[i] * k + i;
    return t;
  endfunction

  initial begin
    automatic int n = 0;
    automatic int acc = 0;
    automatic int a3 [2:0] = '{3, 2, 1};
    automatic int k = 5;
    foreach (m[i, j, b, l]) begin
      m[i][j][b][l] = 1'(i + j + b + l);
      n++;
    end
    $display("n=%0d m[-1][2]=%b m[1][0]=%b", n, m[-1][2], m[1][0]);
    foreach (m[i, , b]) acc += i * 10 + b;
    $display("omitted=%0d", acc);
    s[0] = 8'h5a;
    foreach (s[i]) $display("s[%0d]=%h", i, s[i]);
    foreach (r[i]) r[i] = i;
    foreach (r[i]) $write("%0d ", r[i]);
    $display();
    $display("formal=%0d", formal_sum(a3, k));
    foreach (r[i]) acc = r.sum() with (item * i + k);
    $display("nested=%0d", acc);
    foreach (m[i]) $display("row %0d sum=%0d", i, m[i].sum() with (int'(item[0]) + i));
    $display("index=%0d", r.sum() with (item.index * item.index(1)));
    $finish(0);
  end
endmodule
