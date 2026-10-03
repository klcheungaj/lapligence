// SV2009 13.5.2, 7.2, 7.4: ref/const-ref/output forwarding through nested
// calls, and legal selected actuals (elements, rows and members of unpacked
// arrays and structures, including runtime indices bound once at the call).
module tb;
  typedef logic [7:0] arr_t [0:3];
  typedef logic [7:0] row_t [0:1];
  typedef struct { logic [3:0] a; arr_t m; } rec_t;
  typedef struct { row_t r; logic [3:0] tag; } item_t;
  int k;

  function automatic void inner(ref arr_t x, input int i);
    x[i] = x[i] + 8'd1;
  endfunction
  function automatic void outer(ref arr_t x);
    inner(x, 0);
    inner(x, 3);
  endfunction
  function automatic int csum(const ref arr_t x);
    csum = 0;
    foreach (x[i]) csum += x[i];
  endfunction
  function automatic int cfwd(const ref arr_t x);
    return csum(x) + 1;
  endfunction
  function automatic void ofwd(output arr_t o);
    arr_t t;
    t = '{4{8'h0a}};
    o = t;
  endfunction
  function automatic void ofwd2(output arr_t o);
    ofwd(o);
    o[1] = 8'hbb;
  endfunction
  function automatic void recfwd(ref rec_t r);
    outer(r.m);
    r.a = 4'hf;
  endfunction
  function automatic void bump_row(ref row_t x);
    x[0]++;
    k = 2;
  endfunction
  function automatic void tag(ref logic [3:0] t, input logic [3:0] v);
    t = v;
  endfunction
  function automatic void tag_twice(ref logic [3:0] t);
    tag(t, 4'h3);
    t = t + 4'h4;
  endfunction

  arr_t a;
  arr_t mm [0:1];
  rec_t r;
  row_t rows [0:2];
  item_t items [0:1];
  initial begin
    a = '{8'd1, 8'd2, 8'd3, 8'd4};
    outer(a);
    $display("ref_chain %0d %0d %0d %0d", a[0], a[1], a[2], a[3]);
    $display("const_chain %0d", cfwd(a));
    ofwd2(a);
    $display("output_chain %h %h %h %h", a[0], a[1], a[2], a[3]);
    mm[0] = '{8'd5, 8'd6, 8'd7, 8'd8};
    mm[1] = '{8'd9, 8'd10, 8'd11, 8'd12};
    outer(mm[1]);
    $display("row_ref %0d %0d %0d", mm[1][0], mm[1][3], mm[0][0]);
    $display("row_const %0d", cfwd(mm[0]));
    r.a = 4'h0;
    r.m = '{8'd20, 8'd21, 8'd22, 8'd23};
    recfwd(r);
    $display("member_ref %h %0d %0d", r.a, r.m[0], r.m[3]);
    foreach (rows[i]) rows[i] = '{8'(i * 10), 8'(i * 10 + 1)};
    k = 1;
    bump_row(rows[k]);
    $display("runtime_row %0d %0d %0d %0d", rows[0][0], rows[1][0], rows[2][0], k);
    items[0] = '{'{8'd1, 8'd2}, 4'h0};
    items[1] = '{'{8'd3, 8'd4}, 4'h0};
    k = 1;
    tag(items[k].tag, 4'h9);
    k = 0;
    bump_row(items[k].r);
    tag_twice(items[0].tag);
    $display("element_member %h %h %0d %0d", items[0].tag, items[1].tag, items[0].r[0], items[1].r[0]);
    $finish(0);
  end
endmodule
