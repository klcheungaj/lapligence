// IEEE 1800-2009 12.7.1 with 11.11: a for-loop step's value is discarded, so
// an overloaded increment or compound assignment there runs like the same
// update in an expression statement, for packed-capacity, native (string
// member) and descriptor-sized (65,537-element) targets.
module tb;
  localparam int N = 65537;
  typedef struct { int a; logic [7:0] b; } s_t;
  typedef struct { string s; int n; } n_t;
  typedef int vec_t [0:N-1];
  function automatic s_t inc(s_t x);
    inc.a = x.a + 1;
    inc.b = x.b + 2;
  endfunction
  function automatic s_t add(s_t x, s_t y);
    add.a = x.a + y.a;
    add.b = x.b + y.b;
  endfunction
  function automatic n_t ninc(n_t x);
    ninc.s = {x.s, "+"};
    ninc.n = x.n + 1;
  endfunction
  function automatic n_t ndec(n_t x);
    ndec.s = {x.s, "-"};
    ndec.n = x.n - 1;
  endfunction
  function automatic n_t nadd(n_t x, int y);
    nadd.s = {x.s, "*"};
    nadd.n = x.n + y;
  endfunction
  function automatic vec_t vinc(vec_t v);
    vec_t r;
    foreach (r[i]) r[i] = v[i] + 1;
    return r;
  endfunction
  function automatic vec_t vadd(vec_t v, int y);
    vec_t r;
    foreach (r[i]) r[i] = v[i] + y;
    return r;
  endfunction
  bind ++ function s_t inc(s_t);
  bind + function s_t add(s_t, s_t);
  bind ++ function n_t ninc(n_t);
  bind -- function n_t ndec(n_t);
  bind + function n_t nadd(n_t, int);
  bind ++ function vec_t vinc(vec_t);
  bind + function vec_t vadd(vec_t, int);
  s_t x, y, d;
  s_t sa [3];
  n_t n;
  vec_t c;
  int i, j;

  task automatic bump_native();
    n++;
  endtask

  initial begin
    x.a = 1;
    x.b = 10;
    d.a = 5;
    d.b = 1;
    for (i = 0; i < 3; x++) j = i++;
    $display("packed %0d %0d %0d", x.a, x.b, i);
    for (i = 0; i < 2; i++, x += d);
    $display("packed compound %0d %0d %0d", x.a, x.b, i);
    for (i = 0; i < 2; y = x++) i++;
    $display("packed value %0d %0d | %0d %0d", x.a, x.b, y.a, y.b);
    sa[1].a = 7;
    sa[1].b = 3;
    for (i = 0; i < 2; sa[1]++, i++);
    $display("packed element %0d %0d", sa[1].a, sa[1].b);

    n.s = "a";
    n.n = 1;
    for (i = 0; i < 3; n++) i++;
    $display("native %s %0d %0d", n.s, n.n, i);
    for (i = 0; i < 2; n += 5, i++);
    $display("native compound %s %0d", n.s, n.n);
    for (i = 3; i > 1; i--, --n);
    $display("native prefix %s %0d", n.s, n.n);

    foreach (c[k]) c[k] = k;
    for (i = 0; i < 3; c++) i++;
    $display("descriptor %0d %0d %0d", c[0], c[N-1], i);
    for (i = 0; i < 2; c += 10, i++);
    $display("descriptor compound %0d %0d", c[1], c[N-1]);

    // Expression statements in nested statement positions keep the form.
    if (i == 2) n++;
    case (i)
      2: c++;
      default: ;
    endcase
    begin
      n += 2;
      c += 1;
    end
    fork
      n++;
    join
    bump_native();
    $display("statements %s %0d | %0d %0d", n.s, n.n, c[0], c[N-1]);
    $finish(0);
  end
endmodule
