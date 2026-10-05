// RTL-101b: declaration and member initializers of column-layout records.
module tb;
  typedef struct {
    logic [7:0] a [0:65536] = '{default: 8'h5};
    logic [3:0] tag = 4'h2;
  } in_t;
  typedef struct {
    in_t inner = '{a: '{default: 8'h9}, tag: 4'h7};
    bit [7:0] b [0:65536] = '{default: 8'hx1};
  } out_t;
  typedef struct { logic [7:0] a [0:65536]; logic [3:0] tag; } rec_t;
  in_t d;
  out_t o;
  rec_t m = '{a: '{default: 8'h6}, tag: 4'h1};
  rec_t n = m;
  function automatic int bump(input int k);
    static rec_t s = '{a: '{default: 8'h5}, tag: 4'h2};
    s.tag = s.tag + 4'h1;
    s.a[k] = s.a[k] + 8'h1;
    return int'(s.tag) + int'(s.a[k]);
  endfunction
  function automatic int fresh(input int k);
    out_t l;
    fresh = int'(l.inner.a[k]) + int'(l.b[k]) + int'(l.inner.tag);
    l.b[k] = 8'h0;
  endfunction
  initial begin
    static rec_t s = '{a: '{default: 8'h3}, tag: 4'h4};
    automatic rec_t t = m;
    m.tag = 4'h8;
    $display("A %h %h %h %h", d.a[65536], d.tag, o.inner.a[0], o.inner.tag);
    $display("B %h", o.b[7]);
    $display("C %h %h %h %h", m.a[9], m.tag, n.a[9], n.tag);
    $display("D %0d %0d", bump(3), bump(3));
    $display("E %0d", fresh(4));
    $display("F %h %h %h %h", s.a[1], s.tag, t.a[2], t.tag);
    d.tag = 4'h0;
    o.b[0] = 8'h0;
    $finish(0);
  end
endmodule
