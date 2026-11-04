// Nonblocking writes to records with native members in static storage of a
// subroutine: leaves, selects, sub-records, whole records and container members.
module tb;
  typedef struct {string t; int y;} in_t;
  typedef struct {string s; int n; logic [7:0] v; real r; in_t in; int q[$]; string a[2];} rec_t;
  rec_t seen;
  rec_t g;
  task static keep(input string v, input int k);
    rec_t held;
    static rec_t st = '{"init", 1, 8'h0, 0.0, '{"", 0}, '{}, '{"", ""}};
    int j = 1;
    held = g;
    #1 held.n <= k + 1;
    held.s <= v;
    held.r <= 1.5;
    held.v[3:0] <= 4'hA;
    held.v[7] <= 1'b1;
    held.in <= '{"sub", 5};
    held.in.t <= "subt";
    held.a[1] <= "e1";
    $display("%0d [%s] %0.1f %b", held.n, held.s, held.r, held.v);
    #1 $display("%0d [%s] %0.1f %b [%s] %0d [%s]", held.n, held.s, held.r, held.v, held.in.t, held.in.y, held.a[1]);
    // whole write between issue and commit
    held.n <= 40;
    held = g;
    $display("%0d", held.n);
    #1 $display("%0d [%s]", held.n, held.s);
    // whole NBA with pattern / other record, delay
    held <= g;
    held.n <= 41;
    #1 $display("%0d [%s] %0d", held.n, held.s, held.q.size());
    held <= #2 '{"late", 7, 8'h7, 2.0, '{"x", 1}, '{1, 2}, '{"p", "q"}};
    #1 $display("%0d", held.n);
    #2 $display("%0d [%s] %0d %s", held.n, held.s, held.q.size(), held.a[1]);
    // container member
    held.q <= '{3, 4, 5};
    #1 $display("%0d", held.q.size());
    st.n <= st.n + 1;
    #1 $display("%0d", st.n);
    seen = held;
  endtask
  initial begin
    g.s = "g"; g.n = 9; g.q.push_back(1);
    keep("a", 1);
    $display("[%s] %0d", seen.s, seen.n);
    $finish(0);
  end
endmodule
