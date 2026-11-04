// Nonblocking writes to members of fixed-array record elements.
module tb;
  class C; int v; function new(int x); v = x; endfunction endclass
  typedef struct { string t; int y; } in_t;
  typedef struct {
    string s; int n; logic [7:0] v; real r; C h; logic [3:0][7:0] w;
    int a[2]; string b[2]; in_t in;
  } rec_t;
  rec_t nb[3];
  rec_t g[2][2];
  in_t src;
  int k = 1, i = 0;
  function automatic int bump(); i++; return i; endfunction
  task setm(int e, int v);
    nb[e].n <= v;
    nb[e].s <= "t";
  endtask
  always @(nb[1].n) $display("%0d change n %0d", $time, nb[1].n);
  initial begin
    // (1) Every leaf kind commits in the NBA region.
    #1 nb[1].n <= 9;
    nb[1].s <= "new";
    nb[1].r <= 2.5;
    nb[1].h <= new(7);
    nb[1].v <= 8'hA5;
    nb[1].a[1] <= 4;
    $display("%0d [%s] %0.1f %0d", nb[1].n, nb[1].s, nb[1].r, nb[1].h == null);
    #1 $display("%0d [%s] %0.1f %0d %h %0d", nb[1].n, nb[1].s, nb[1].r, nb[1].h.v, nb[1].v, nb[1].a[1]);
    // (2) The position is fixed at issue; the commit reaches the element a
    // blocking whole-element write stored in between.
    nb[k].n <= 11;
    nb[k] = '{s:"w", n:2, v:8'h00, r:0.0, h:null, w:0, a:'{0, 0}, b:'{"", ""}, in:'{"", 0}};
    k = 2;
    #1 $display("%0d [%s] %0d", nb[1].n, nb[1].s, nb[2].n);
    // (3) Bit and part selects; the last write to a bit wins.
    nb[1].v[2] <= 1'b1;
    nb[1].v[7:6] <= 2'b11;
    nb[1].w[1][3:0] <= 4'hF;
    nb[1].v[0] <= 1'b1;
    nb[1].v[0] <= 1'b0;
    #1 $display("%b %h", nb[1].v, nb[1].w);
    // (4) Intra-assignment delays.
    nb[0].n <= #3 5;
    nb[0].s <= #2 "d";
    nb[0].v[1] <= #1 1'b1;
    #1 $display("%0d [%s] %b", nb[0].n, nb[0].s, nb[0].v);
    #1 $display("%0d [%s]", nb[0].n, nb[0].s);
    #1 $display("%0d", nb[0].n);
    #1 $display("%0d", nb[0].n);
    // (5) Two dimensions; an index with a side effect runs once.
    g[1][k-1].n <= 3;
    g[k-1][0].s <= "gg";
    nb[bump()].n <= 21;
    #1 $display("%0d [%s] %0d %0d", g[1][1].n, g[1][0].s, nb[1].n, i);
    // (6) An out-of-range position writes nothing.
    k = 7;
    nb[k].n <= 1;
    #1 $display("%0d %0d %0d", nb[0].n, nb[1].n, nb[2].n);
    // (7) Queued writes commit in issue order with whole-element writes.
    nb[1] <= '{s:"a", n:1, v:8'h1, r:0.0, h:null, w:0, a:'{0, 0}, b:'{"", ""}, in:'{"", 0}};
    nb[1].n <= 5;
    #1 $display("%0d [%s]", nb[1].n, nb[1].s);
    nb[1].n <= 6;
    nb[1] <= '{s:"b", n:2, v:8'h2, r:0.0, h:null, w:0, a:'{0, 0}, b:'{"", ""}, in:'{"", 0}};
    #1 $display("%0d [%s]", nb[1].n, nb[1].s);
    // (8) From a task.
    k = 1;
    setm(k, 4);
    #1 $display("%0d [%s]", nb[1].n, nb[1].s);
    // (9) Sub-records and member arrays take their value at issue.
    src = '{"srcv", 3};
    nb[k].in <= src;
    src.t = "changed";
    nb[0].a <= '{4, 5};
    nb[0].b <= '{"p", "q"};
    $display("[%s] %0d", nb[1].in.t, nb[1].in.y);
    k = 0;
    #1 $display("[%s] %0d", nb[1].in.t, nb[1].in.y);
    $display("%0d %0d %s %s", nb[0].a[0], nb[0].a[1], nb[0].b[0], nb[0].b[1]);
    nb[1].in <= #2 '{"d", 9};
    nb[1].s = "z";
    #3 $display("[%s] %0d [%s]", nb[1].in.t, nb[1].in.y, nb[1].s);
    $finish(0);
  end
endmodule
