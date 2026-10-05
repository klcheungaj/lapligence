// SIM-006 with RTL-101: a function whose formals include column-layout
// records (a member array beyond the dense threshold) and whose result is a
// container or a real array (SV 7.5, 7.10, 13.4). The record's extra columns
// and the result are both trailing output formals; the call must order them
// as the signature does, whether the result is assigned or discarded.
module tb;
  typedef struct { bit [7:0] a [0:4999]; bit [7:0] t; } rec_t;
  typedef int iq_t[$];
  typedef real ra_t [0:1];
  rec_t r, o, io;
  int b[$];
  int s[$];
  ra_t v;

  function automatic iq_t cols(input rec_t x, output rec_t y, inout rec_t z,
                               input int extra[$]);
    iq_t q;
    q.push_back(int'(x.a[4999]));
    q.push_back(int'(x.t));
    q.push_back(int'(z.t));
    q.push_back(extra.size());
    y = x;
    y.a[1] = 8'd77;
    z.t = z.t + 8'd1;
    return q;
  endfunction

  function automatic iq_t ins(input rec_t x, input int n);
    iq_t q;
    for (int i = 0; i < n; i++) q.push_back(int'(x.a[i]) + int'(x.t));
    return q;
  endfunction

  function automatic ra_t rr(input rec_t x, output rec_t y);
    ra_t q;
    q[0] = real'(x.t) * 0.5;
    q[1] = 2.5;
    y = x;
    y.t = x.t + 8'd1;
    return q;
  endfunction

  initial begin
    r.a[4999] = 8'd5;
    r.t = 8'd3;
    r.a[0] = 8'd10;
    r.a[1] = 8'd20;
    io.t = 8'd40;
    s = '{1, 2};
    b = cols(r, o, io, s);
    $display("A %0d %0d %0d %0d %0d", b.size(), b[0], b[1], b[2], b[3]);
    $display("B %0d %0d %0d", o.a[1], o.a[4999], io.t);
    void'(cols(r, o, io, s));
    $display("C %0d %0d", io.t, b.size());
    b = ins(r, 2);
    $display("D %0d %0d %0d", b.size(), b[0], b[1]);
    r.t = 8'd7;
    v = rr(r, o);
    $display("E %0.1f %0.1f %0d", v[0], v[1], o.t);
    r.t = 8'd9;
    void'(rr(r, o));
    $display("F %0d %0.1f", o.t, v[0]);
    $finish(0);
  end
endmodule
