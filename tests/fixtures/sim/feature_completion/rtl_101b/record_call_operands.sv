// RTL-101b: calls returning column-layout records are compared and
// selected from inside expressions; each call runs once per operand.
module tb;
  typedef struct { logic [7:0] a [0:65536]; logic [3:0] k; } in_t;
  typedef struct {
    bit [1023:0] w [0:2047];
    in_t s;
    real x;
    string n;
    bit [7:0] t;
  } big_t;
  typedef union tagged {
    logic [1023:0] w [0:2047];
    bit [7:0] t;
  } tu_t;
  big_t r;
  int calls;
  function automatic big_t f(input big_t b, input bit [7:0] d);
    calls = calls + 1;
    b.t = b.t + d;
    b.x = b.x + 0.5;
    return b;
  endfunction
  function automatic tu_t g(input bit [7:0] d);
    calls = calls + 1;
    return tagged t d;
  endfunction
  function automatic int depth(input big_t b);
    int n;
    n = 0;
    while (f(b, 8'd0).t < 8'd5) begin
      b.t = b.t + 8'd1;
      n = n + 1;
    end
    return n;
  endfunction
  initial begin
    calls = 0;
    r.t = 8'd1;
    r.x = 1.0;
    r.n = "q";
    r.s.k = 4'h6;
    r.s.a[4] = 8'h44;
    r.w[2] = 1024'd5;
    $display("A %0d %0d %0d", f(r, 8'd0) == r, f(r, 8'd1) == f(r, 8'd1), calls);
    if (f(r, 8'd2) != r)
      $display("B %0d %h %h %0d", f(r, 8'd2).t, f(r, 8'd0).s.k, f(r, 8'd0).s.a[4], calls);
    $display("C %f %0d %0d", f(r, 8'd0).x, f(r, 8'd0).w[2][7:0], depth(r));
    $display("D %0d %0d %0d", g(8'd9).t, g(8'd9) == g(8'd9), g(8'd9) != g(8'd8));
    $finish(0);
  end
endmodule
