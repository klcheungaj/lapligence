// A `ref` formal of the column form bound to one record image inside a column
// cell or an array of records: any index selects it, forwarding and nested
// elements reach the same cell, and the cell sees each write at once while the
// task waits on it (SV 13.5.2, 9.4.2).
`ifndef N
`define N 16
`endif
module tb;
  typedef struct { byte n; shortint z [2]; } sub_t;
  typedef struct { bit [3:0] a; logic [3:0] b; byte v [3]; sub_t s [`N]; } row_t;
  typedef struct { logic [3:0] h; row_t rows [`N]; bit [7:0] t; } rec_t;
  rec_t r;
  row_t x;
  row_t plain [4];
  int i;
  task automatic poke(ref sub_t q, input byte k);
    q.n = k;
    q.z[1] = k + 1;
  endtask
  task automatic through_ref(ref row_t v, input int k);
    v.b = 4'hd;
    v.v[2] = 99;
    v.v[i] = 4;
    v.s[k].n = 8'(k);
    poke(v.s[k + 1], 8'(k + 10));
  endtask
  task automatic forward(ref row_t v);
    through_ref(v, 2);
    v.a = 5;
  endtask
  function automatic int reader(const ref row_t v);
    return int'(v.a) + int'(v.v[2]);
  endfunction
  task automatic whole(ref row_t v, input row_t o);
    v = o;
    x = v;
  endtask
  task automatic waiter(ref row_t v);
    @(v.b);
    $display("woke %h", v.b);
    v.a = 2;
    #1 $display("late %h", v.b);
  endtask
  initial begin
    i = 1;
    through_ref(r.rows[7], 3);
    $display("ref %h %0d %0d %0d %0d %0d", r.rows[7].b, r.rows[7].v[2], r.rows[7].v[1], r.rows[7].s[3].n, r.rows[7].s[4].n, r.rows[7].s[4].z[1]);
    through_ref(r.rows[i], 5);
    $display("ref %h %0d %0d %0d", r.rows[1].b, r.rows[1].v[2], r.rows[1].v[1], r.rows[1].s[5].n);
    forward(r.rows[2]);
    $display("fwd %h %h %0d %0d", r.rows[2].a, r.rows[2].b, r.rows[2].v[2], reader(r.rows[2]));
    forward(x);
    $display("var %h %h %0d", x.a, x.b, reader(x));
    poke(r.rows[3].s[6], 44);
    $display("sub %0d %0d", r.rows[3].s[6].n, r.rows[3].s[6].z[1]);
    x.a = 9;
    whole(r.rows[4], x);
    $display("whole %h %h %0d", r.rows[4].a, x.b, r.rows[4].v[2]);
    forward(plain[i]);
    $display("plain %h %h %0d", plain[1].a, plain[1].b, reader(plain[1]));
    fork
      waiter(r.rows[5]);
      begin #1 r.rows[5].b = 4'h3; #1 r.rows[5].b = 4'h6; end
    join
    $display("wait %h %h", r.rows[5].a, r.rows[5].b);
    $finish;
  end
endmodule
