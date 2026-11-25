// Conditional chains over containers and records: predicates and alternatives with side effects, and a target that is one of its alternatives (IEEE 1800-2009 11.4.11).

module tb;
  typedef struct { int a; string t; } rec_t;
  typedef logic [7:0] q_t[$];
  logic [7:0] s;
  string trace;
  q_t y, z;
  rec_t r;
  function automatic logic p(input int i);
    trace = {trace, $sformatf(" p%0d", i)};
    return s[i];
  endfunction
  function automatic q_t f(input int i);
    q_t v;
    trace = {trace, $sformatf(" f%0d", i)};
    v.push_back(i % 2 ? 8'h5a : 8'ha5);
    if (i == 3) v.push_back(8'h77);
    return v;
  endfunction
  function automatic rec_t g(input int i);
    rec_t v;
    trace = {trace, $sformatf(" g%0d", i)};
    v.a = i % 2 ? 32'h5a5a5a5a : 32'ha5a5a5a5;
    v.t = $sformatf("t%0d", i);
    return v;
  endfunction
  task automatic show(input string tag, input int i);
    $write("%s %0d:%s =", tag, i, trace);
    foreach (y[j]) $write(" %h", y[j]);
    $write("\n");
  endtask
  task automatic check(input string tag, input int n, i);
    trace = "";
y = p(0) ? f(0) : p(1) ? f(1) : p(2) ? f(2) : p(3) ? f(3) : p(4) ? f(4) : p(5) ? f(5) : p(6) ? f(6)
        : p(7) ? f(7) : f(8);
    show(tag, i);
    trace = ""; y = '{8'h01, 8'h02};
y = p(0) ? y : p(1) ? {y, 8'h03} : p(2) ? y : p(3) ? {y, 8'h03} : p(4) ? y : p(5) ? {y, 8'h03} :
        p(6) ? y : p(7) ? {y, 8'h03} : y;
    show({tag, " self"}, i);
    trace = ""; y = '{8'h01, 8'h02}; z = '{8'h01, 8'hfd, 8'h05};
y = s[0] ? y : s[1] ? z : s[2] ? y : s[3] ? z : s[4] ? y : s[5] ? z : s[6] ? y : s[7] ? z : y;
    show({tag, " both"}, i);
    trace = "";
r = p(0) ? g(0) : p(1) ? g(1) : p(2) ? g(2) : p(3) ? g(3) : p(4) ? g(4) : p(5) ? g(5) : p(6) ? g(6)
        : p(7) ? g(7) : g(8);
    $display("%s %0d:%s = %h", tag, i, trace, r.a);
  endtask
  // Predicates: one of them 1, X or Z; an X followed by a 1; an X and a Z two further on; none; all X.
  task automatic drive(input int n);
    for (int i = 0; i < n; i++) begin
      s = '0; s[i] = 1'b1; check("1", n, i);
      s[i] = 1'bx; check("x", n, i);
      s[i] = 1'bz; check("z", n, i);
      if (i + 1 < n) begin s[i] = 1'bx; s[i + 1] = 1'b1; check("x1", n, i); s[i + 1] = 1'b0; end
      if (i + 2 < n) begin s[i] = 1'bx; s[i + 2] = 1'bz; check("xz", n, i); end
    end
    s = '0; check("none", n, 0);
    s = 'x; check("allx", n, 0);
  endtask
  initial drive(8);
endmodule
