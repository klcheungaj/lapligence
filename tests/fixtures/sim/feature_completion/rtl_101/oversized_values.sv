// RTL-101: a single record wider than the packed limit moves as a value.
typedef struct { logic [1023:0] w [0:2047]; bit [7:0] t; } big_t;

module pass_through(input big_t i, output big_t o);
  assign o = i;
endmodule

module stamp(ref big_t x);
  initial #5 begin
    x.w[9] = 1024'd99;
    x.t = 8'd42;
  end
endmodule

module tb;
  big_t r, s, u, q, z;
  logic sel;
  function automatic big_t f(input big_t x);
    big_t y;
    y = x;
    y.t = 8'd9;
    return y;
  endfunction
  function automatic void g(output big_t o, input big_t i, inout big_t io);
    o = i;
    o.w[1] = 1024'd77;
    io.t = io.t + 8'd1;
  endfunction
  function automatic void h(ref big_t v);
    v.w[2] = 1024'd55;
    v.t = 8'd200;
  endfunction
  function big_t st(input big_t x);
    st = x;
    st.t = st.t + 8'd3;
  endfunction
  function automatic int depth(input big_t x, input int n);
    big_t y;
    if (n == 0) return int'(x.t);
    y = x;
    y.t = y.t + 8'd1;
    return depth(y, n - 1);
  endfunction
  function automatic big_t mk(input bit [7:0] v);
    return '{w: '{default: {1016'd0, v}}, t: v};
  endfunction
  pass_through p(.i(r), .o(q));
  stamp m(r);
  assign z = q;
  initial begin
    r.w[3] = 5;
    r.t = 1;
    s = f(r);
    $display("A %0d %0d %0d %0d", s.w[3][7:0], s.t, r == s, r != s);
    sel = 1'bx;
    u = sel ? r : s;
    $display("B %0d %0d", u.w[3][7:0], u.t);
    g(u, r, s);
    $display("C %0d %0d %0d %0d", u.w[1][7:0], u.w[3][7:0], u.t, s.t);
    h(u);
    $display("D %0d %0d", u.w[2][7:0], u.t);
    u = st(r);
    $display("E %0d %0d", u.t, depth(r, 5));
    #1 $display("F %0d %0d %0d", q.w[3][7:0], q.t, z.t);
    s <= mk(8'd6);
    #1 $display("G %0d %0d %0d", s.w[0][7:0], s.w[2047][7:0], s.t);
    #5 $display("H %0d %0d %0d %0d", r.w[9][7:0], r.t, q.w[9][7:0], z.t);
    $finish(0);
  end
endmodule
