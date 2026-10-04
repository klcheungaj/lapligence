// IEEE 1800-2009 11.11 on fixed unpacked arrays wider than the packed value
// capacity: overloaded operands, results and compound targets cross the bound
// functions as descriptors, never as flattened packed values.
module tb;
  localparam int N = 65537;
  typedef int vec_t [0:N-1];

  function automatic vec_t vadd(vec_t a, vec_t b);
    vec_t r;
    foreach (r[i]) r[i] = a[i] + b[i];
    return r;
  endfunction

  function automatic vec_t vneg(vec_t a);
    vec_t r;
    foreach (r[i]) r[i] = -a[i];
    return r;
  endfunction

  function automatic vec_t vfill(int x);
    vec_t r;
    foreach (r[i]) r[i] = x;
    return r;
  endfunction

  bind + function vec_t vadd(vec_t, vec_t);
  bind - function vec_t vneg(vec_t);
  bind = function vec_t vfill(int);

  vec_t a, b, c;

  initial begin
    foreach (a[i]) begin
      a[i] = i;
      b[i] = 2 * i;
    end
    c = a + b;
    $display("add %0d %0d %0d", c[0], c[1], c[N-1]);
    c = -c;
    $display("neg %0d %0d", c[1], c[N-1]);
    c += a;
    $display("comp %0d %0d", c[1], c[N-1]);
    c = 7;
    $display("fill %0d %0d", c[0], c[N-1]);
    $display("eq %0d %0d", a == a, a == b);
    $finish;
  end
endmodule
