// Conditional operators over records in the arguments of calls in their alternatives, 5 to 8 levels deep (IEEE 1800-2009 11.4.11).

module tb;
  typedef struct { int a; string t; } rec_t;
  logic [7:0] patterns [] = '{8'b00000001, 8'b00000010, 8'b00000100, 8'b00001000, 8'b00010000, 8'b00100000, 8'b01000000, 8'b10000000, 8'b0000000x, 8'b000000x0, 8'b00000x00, 8'b0000x000, 8'b000x0000, 8'b00x00000, 8'b0x000000, 8'bx0000000, 8'b0000001x, 8'b000001x0, 8'b00001x00, 8'b0001x000, 8'b001x0000, 8'b01x00000, 8'b1x000000, 8'b00000x0z, 8'b0000x0z0, 8'b000x0z00, 8'b00x0z000, 8'b0x0z0000, 8'bx0z00000, 8'b00000000, 8'bxxxxxxxx, 8'bx1x1x1x1, 8'b1x1x1x1x, 8'bx0x0x0x0, 8'b0x0x0x0x, 8'bxx00xx00, 8'bzzzzzzz1};
  logic [7:0] s;
  string trace;
  rec_t d0, d1, d2, d3, d4, d5, d6, d7, d8, y, z;
  function automatic rec_t f(input rec_t v);
    trace = {trace, $sformatf(" f%0d", v.a)};
    v.a += 1;
    return v;
  endfunction
  function automatic rec_t g(input rec_t v);
    trace = {trace, $sformatf(" g%0d", v.a)};
    d0.a += 10;
    d1.t = {d1.t, "!"};
    y.a += 100;
    v.a += 1;
    return v;
  endfunction
  function automatic rec_t local_target();
    rec_t r;
    r = s[0] ? f(s[1] ? f(s[2] ? f(s[3] ? f(s[4] ? f(s[5] ? f(s[6] ? f(s[7] ? f(d8) : d7) : d6) : d5) : d4) : d3) : d2) : d1) : d0;
    r.t = {r.t, "."};
    return r;
  endfunction
  task automatic reset();
    trace = "";
    d0 = '{0, "t0"};
    d1 = '{1, "t1"};
    d2 = '{2, "t0"};
    d3 = '{0, "t1"};
    d4 = '{1, "t0"};
    d5 = '{2, "t1"};
    d6 = '{0, "t0"};
    d7 = '{1, "t1"};
    d8 = '{2, "t0"};
    y = '{7, "y"};
  endtask
  task automatic show(input string tag);
    $display("%s %b:%s = %0d \"%s\", d0 %0d, d1 \"%s\"", tag, s, trace, y.a, y.t, d0.a, d1.t);
  endtask
  task automatic start(input int i);
    s = patterns[i];
    reset();
  endtask
  task automatic show_z();
    y = z;
    show("nonblocking");
  endtask
  task automatic check();
    reset();
    y = s[0] ? d0 : f(s[1] ? d1 : f(s[2] ? d2 : f(s[3] ? d3 : f(s[4] ? d4 : f(d5)))));
    show("last5");
    reset();
    y = s[0] ? f(s[1] ? f(s[2] ? f(s[3] ? f(s[4] ? f(s[5] ? f(s[6] ? f(s[7] ? f(d8) : d7) : d6) : d5) : d4) : d3) : d2) : d1) : d0;
    show("first8");
    reset();
    y = s[0] ? f(s[1] ? d1 : f(s[2] ? f(s[3] ? d3 : f(s[4] ? f(s[5] ? d5 : f(d6)) : d4)) : d2)) : d0;
    show("alt6");
    reset();
    y = s[0] ? d0 : g(s[1] ? d1 : g(s[2] ? d2 : g(s[3] ? d3 : g(s[4] ? d4 : g(s[5] ? d5 : g(d6))))));
    show("effects6");
    reset();
    y = s[0] ? y : f(s[1] ? d1 : f(s[2] ? y : f(s[3] ? d3 : f(s[4] ? y : f(y)))));
    show("self5");
    reset();
    y = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : g(s[4] ? d4 : f(s[5] ? d5 : f(s[6] ? d6 : f(s[7] ? d7 : f(d8)))));
    show("chain8");
    reset();
    y = s[0] ? f(d0) : g(s[1] ? d1 : g(s[2] ? d2 : g(s[3] ? d3 : g(s[4] ? d4 : g(s[5] ? d5 : g(s[6] ? d6 : g(d7)))))));
    show("leaf7");
    reset();
    y = s[0] ? g(s[1] ? d1 : f(s[2] ? d2 : f(s[3] ? d3 : f(s[4] ? d4 : f(s[5] ? d5 : f(s[6] ? d6 : f(d7))))))) : f(s[1] ? g(s[2] ? g(s[3] ? g(s[4] ? g(s[5] ? g(s[6] ? g(d7) : d6) : d5) : d4) : d3) : d2) : d1);
    show("both7");
    reset();
    y = local_target();
    show("local");
  endtask
  initial begin
    foreach (patterns[i]) begin
      start(i);
      check();
    end
    foreach (patterns[i]) begin
      start(i);
      z <= s[0] ? d0 : f(s[1] ? d1 : f(s[2] ? d2 : f(s[3] ? d3 : f(s[4] ? d4 : f(d5)))));
      #1;
      show_z();
    end
  end
endmodule
