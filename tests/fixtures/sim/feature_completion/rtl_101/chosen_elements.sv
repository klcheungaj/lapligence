// A record element of a member array is one column cell: its members select
// from that cell, and the element moves as a value between arrays of records,
// record variables, arguments and streams (SV 7.2, 7.4, 7.6, 11.4.14). A read
// at an invalid index gives each member its default-uninitialized value
// (SV 7.4.6), so the state domain of each member survives.
module tb;
  localparam int N = 16;
  typedef struct { bit [3:0] a; logic [3:0] b; byte v [3]; } row_t;
  typedef struct { logic [3:0] h; row_t rows [N]; bit [7:0] t; } rec_t;
  typedef struct { row_t row; int n; } wrap_t;
  rec_t r, s;
  row_t x, y;
  row_t plain [4];
  wrap_t w;
  logic [31:0] image;
  int i, j;

  function automatic int sum(row_t v);
    return int'(v.a) + int'(v.v[0]) + int'(v.v[2]);
  endfunction

  function automatic row_t mk(int k);
    row_t v;
    v.a = 4'(unsigned'(k));
    v.b = 4'(unsigned'(k + 1));
    v.v[0] = byte'(k + 2);
    v.v[1] = byte'(k + 3);
    v.v[2] = byte'(k + 4);
    return v;
  endfunction

  task automatic bump(inout row_t v, output row_t o);
    o = v;
    v.a = v.a + 1;
    o.v[1] = -1;
  endtask

  task automatic through_ref(ref row_t v);
    v.b = 4'hd;
    v.v[2] = 99;
  endtask

  initial begin
    i = 3;
    j = 1;
    r.rows[i].a = 4'h7;
    r.rows[2].b = 4'hc;
    r.rows[i].v[j] = 21;
    r.rows[2].v[0] = -5;
    $display("members %h %h %0d %0d %h", r.rows[i].a, r.rows[2].b, r.rows[i].v[j],
             r.rows[2].v[0], r.rows[5].a);
    $display("unwritten %b %b %0d", r.rows[1].b, r.rows[1].a, r.rows[1].v[2]);
    r.rows[i + 1].b[2:1] = 2'b10;
    #2;
    $display("selects %b %b", r.rows[4].b, r.rows[4].a);

    r.rows[2] = '{a: 1, b: 2, v: '{3, 4, 5}};
    x = r.rows[2];
    $display("to record %h %h %0d %0d", x.a, x.b, x.v[0], x.v[2]);
    x.a = 9;
    r.rows[3] = x;
    $display("from record %h %0d", r.rows[3].a, r.rows[3].v[1]);
    r.rows[i + 3] = r.rows[2];
    $display("between cells %h %0d", r.rows[6].b, r.rows[6].v[2]);
    plain[j] = r.rows[3];
    r.rows[7] = plain[1];
    $display("plain arrays %h %h", plain[1].a, r.rows[7].a);
    r.rows[8] = mk(1);
    w.row = r.rows[8];
    r.rows[9] = w.row;
    $display("nested %0d %0d", w.row.v[1], r.rows[9].v[2]);

    $display("input %0d %0d", sum(r.rows[8]), sum('{a: 1, b: 0, v: '{1, 1, 1}}));
    bump(r.rows[8], r.rows[10]);
    $display("task %h %h %0d", r.rows[8].a, r.rows[10].a, r.rows[10].v[1]);
    through_ref(r.rows[11]);
    $display("ref %h %0d", r.rows[11].b, r.rows[11].v[2]);

    y = r.rows[9];
    $display("equal %b %b %b %b", r.rows[9] == y, r.rows[9] != y, y === r.rows[9],
             r.rows[9] == r.rows[10]);
    s = r;
    $display("whole %b", s == r);
    s.rows[3].b = 4'bx10z;
    $display("mixed %b %b", s == r, s === r);
    s.rows[4].a = 4'h1;
    r.rows[4].a = 4'h2;
    $display("known mismatch %b", s.rows[3:4] == r.rows[3:4]);

    i = 100;
    x = r.rows[i];
    $display("invalid %b %b %0d", r.rows[i].a, r.rows[i].b, r.rows[i].v[0]);
    $display("invalid value %b %b", x.a, x.b);
    r.rows[i] = mk(1);
    r.rows[i].a = 1;

    {>>{x}} = 32'hab_01_02_03;
    $display("stream target %h %h %0d %0d", x.a, x.b, x.v[0], x.v[2]);
    image = {<<8{r.rows[2]}};
    $display("stream source %h", image);
    $finish(0);
  end

  initial begin
    #1;
    r.rows[4].a[0] <= 1'b1;
  end
endmodule
