// IEEE 1800-2009 7.2, 7.4, 7.6, 10.4.2, 11.4.5 and Table 11-20: a fixed
// array of unpacked mixed-state records whose total width (1,048,576 cells x
// 20 bits) exceeds the 1,048,575-bit packed limit is copied, compared,
// merged, passed through a function and published by NBA as a whole value.
module tb;
  typedef struct { logic [15:0] a; bit [3:0] b; } rec_t;
  typedef rec_t arr_t [0:1048575];
  arr_t m, n, q;
  logic sel;
  function automatic arr_t bump(input arr_t x);
    arr_t y;
    y = x;
    y[7].a = y[7].a + 16'd1;
    return y;
  endfunction
  initial begin
    m[7].a = 16'h0010; m[7].b = 4'h2;
    m[9].a = 16'h0009; m[9].b = 4'h3;
    m[1048575].a = 16'hbeef;
    n = m;
    n[9].a = 16'h0019;
    $display("C %h %h %h %h %h", n[7].a, n[9].a, n[1048575].a, n[0].a, n[0].b);
    $display("E %0d %0d", m == n, m != n);
    sel = 1'bx;
    q = sel ? m : n;
    $display("Q %h %h %h %h", q[7].a, q[7].b, q[9].a, q[9].b);
    q <= bump(m);
    #1 $display("N %h %h %h %h", q[7].a, q[9].a, q[9].b, q[1048575].a);
    $finish(0);
  end
endmodule
