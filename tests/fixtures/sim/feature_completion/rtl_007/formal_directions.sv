// SV2009 13.3-13.5, 6.21: fixed arrays and records in every formal direction,
// returns and locals; static storage persists per declaration, automatic
// storage per activation.
module tb;
  typedef logic [7:0] arr_t [0:3];
  typedef struct { logic [3:0] a; bit [7:0] b; } rec_t;
  typedef struct { rec_t r; arr_t v; } deep_t;

  function automatic arr_t rev(input arr_t x);
    for (int i = 0; i < 4; i++) rev[i] = x[3 - i];
  endfunction

  function automatic rec_t swap(input rec_t r);
    swap.a = r.b[3:0];
    swap.b = {4'h0, r.a};
  endfunction

  function automatic void fill(output arr_t o, input logic [7:0] base);
    foreach (o[i]) o[i] = base + 8'(i);
  endfunction

  function automatic void bump(inout arr_t io, inout rec_t r);
    foreach (io[i]) io[i] = io[i] + 1;
    r.a = r.a + 1;
  endfunction

  function automatic void alias_write(ref arr_t x, ref rec_t r, ref deep_t d);
    x[0] = 8'hee;
    r.b = 8'h77;
    d.r.a = 4'h6;
    d.v[3] = 8'h5a;
  endfunction

  function automatic int csum(const ref arr_t x);
    int s = 0;
    foreach (x[i]) s += x[i];
    return s;
  endfunction

  function automatic deep_t build(input rec_t r, input arr_t v);
    deep_t d;
    d.r = r;
    d.v = v;
    return d;
  endfunction

  task automatic t_out(output rec_t r, output arr_t a);
    r.a = 4'h5;
    r.b = 8'h66;
    a = '{8'd1, 8'd2, 8'd3, 8'd4};
  endtask

  // Static subroutines keep formal, local and result storage per declaration.
  function arr_t accumulate(input arr_t x);
    arr_t total = '{default: 8'd0};
    foreach (total[i]) total[i] = total[i] + x[i];
    return total;
  endfunction

  function void static_out(output arr_t o, input bit set);
    if (set) o = '{8'h11, 8'h22, 8'h33, 8'h44};
  endfunction

  function automatic void auto_out(output arr_t o, input bit set);
    if (set) o = '{8'h11, 8'h22, 8'h33, 8'h44};
  endfunction

  task static_count(output int n);
    static int calls = 0;
    calls++;
    n = calls;
  endtask

  arr_t a, b;
  rec_t r, s;
  deep_t d;
  int n;
  initial begin
    a = '{8'h10, 8'h20, 8'h30, 8'h40};
    b = rev(a);
    $display("rev %h %h %h %h", b[0], b[1], b[2], b[3]);
    r = '{a: 4'h3, b: 8'h9c};
    s = swap(r);
    $display("swap %h %h", s.a, s.b);
    fill(b, 8'h50);
    $display("fill %h %h %h %h", b[0], b[1], b[2], b[3]);
    bump(b, r);
    $display("inout %h %h %h %h %h", b[0], b[1], b[2], b[3], r.a);
    d = build(r, a);
    alias_write(b, r, d);
    $display("ref %h %h %h %h %h", b[0], r.b, d.r.a, d.r.b, d.v[3]);
    $display("const_ref %0d", csum(a));
    t_out(s, a);
    $display("task_out %h %h %0d %0d", s.a, s.b, a[0], a[3]);
    b = accumulate(a);
    b = accumulate(a);
    $display("static_result %0d %0d", b[0], b[3]);
    static_out(b, 1);
    b = '{default: 8'h00};
    static_out(b, 0);
    $display("static_output %h %h", b[0], b[3]);
    b = '{default: 8'h00};
    auto_out(b, 0);
    $display("auto_output %h %h", b[0], b[3]);
    static_count(n);
    static_count(n);
    $display("static_task %0d", n);
    $finish(0);
  end
endmodule
