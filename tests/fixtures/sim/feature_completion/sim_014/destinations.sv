// SIM-014 A01: destinations of timed assignments whose selectors and RHS
// change while the assignment is pending. A blocking assignment evaluates
// its RHS before the timing control and its destination (selectors, and the
// object a handle names) when the control completes (SV 9.4.5, 10.4.1,
// 4.9.3). A nonblocking assignment evaluates both when it is issued
// (SV 10.4.2, 4.9.4). Blocking forms run in the first process, nonblocking
// forms after them in the second; their windows do not overlap.
`timescale 1ns / 1ns
module tb;
  typedef struct packed {
    logic [7:0] f;
    logic [7:0] g;
  } pair_t;
  class Box;
    int x;
  endclass
  event e;
  logic [7:0] m[0:3];
  logic [7:0] n[0:3];
  int i, j;
  pair_t s, t;
  int q[$];
  int dyn[];
  int aa[string];
  string key;
  logic [7:0] b, v, w, nb;
  real r, rr, nr, nrv;
  string st, sv, ns, nsv;
  Box h, h1, h2;

  // `e` occurs at 4, 8, 12, ...; every assignment is issued at 1 mod 4 and
  // its operands change at 2 mod 4.
  initial forever #4 ->e;

  initial begin
    foreach (m[k]) m[k] = 0;
    q = {0, 0, 0, 0};
    dyn = new[4];
    s = '0;
    v = '0;
    #1 i = 0;
    b = 8'h11;
    fork
      m[i] = @e b;
      #1 begin
        i = 2;
        b = 8'h22;
      end
    join
    #1 $display("%0t array m: %h %h %h %h", $time, m[0], m[1], m[2], m[3]);
    i = 0;
    fork
      s.f[i] = @e 1'b1;
      #1 i = 5;
    join
    #1 $display("%0t member s.f=%b", $time, s.f);
    i = 0;
    fork
      v[i+:2] = repeat (2) @e 2'b11;
      #1 i = 6;
    join
    #1 $display("%0t part v=%b", $time, v);
    i = 0;
    fork
      q[i] = repeat (2) @e 7;
      #1 i = 3;
    join
    #1 $display("%0t queue q=%0d %0d %0d %0d", $time, q[0], q[1], q[2], q[3]);
    i = 0;
    fork
      dyn[i] = @e 8;
      #1 i = 1;
    join
    #1 $display("%0t dynamic dyn=%0d %0d", $time, dyn[0], dyn[1]);
    key = "a";
    fork
      aa[key] = @e 9;
      #1 key = "b";
    join
    #1 $display("%0t associative a=%0d b=%0d", $time, aa.exists("a"), aa["b"]);
    h1 = new;
    h2 = new;
    h = h1;
    fork
      h.x = @e 10;
      #1 h = h2;
    join
    #1 $display("%0t handle h1.x=%0d h2.x=%0d", $time, h1.x, h2.x);
    rr = 1.5;
    fork
      r = @e rr * 2.0;
      #1 rr = 7.0;
    join
    #1 $display("%0t real r=%0.2f", $time, r);
    sv = "ab";
    fork
      st = repeat (2) @e sv;
      #1 sv = "zz";
    join
    #1 $display("%0t string st=%s", $time, st);
  end

  // Nonblocking forms: this process only issues; the next one changes the
  // selectors and operands after each issue.
  initial begin
    #53 n[j] <= @e nb;
    #4 $display("%0t nba array n: %h %h %h %h", $time, n[0], n[1], n[2], n[3]);
    t.g[j] <= repeat (2) @e 1'b1;
    #8 $display("%0t nba member t.g=%b", $time, t.g);
    w[j+:2] <= @e 2'b11;
    #4 $display("%0t nba part w=%b", $time, w);
    nr <= @e nrv * 2.0;
    #4 $display("%0t nba real nr=%0.2f", $time, nr);
    ns <= repeat (2) @e {nsv, "!"};
    #8 $display("%0t nba string ns=%s", $time, ns);
    $finish;
  end

  initial begin
    foreach (n[k]) n[k] = 0;
    t = '0;
    w = '0;
    #52 j = 1;
    nb = 8'h33;
    #2 j = 3;
    nb = 8'h44;
    #2 j = 2;
    #2 j = 6;
    #6 j = 1;
    #2 j = 4;
    #2 nrv = 2.5;
    #2 nrv = 0.0;
    #2 nsv = "one";
    #2 nsv = "two";
  end
endmodule
