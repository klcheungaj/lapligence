// Random arrays of enum or packed-structure elements and packed structures
// with enum members (18.4, 6.19, 7.2.1): every random element and member
// holds a named value, with every named value drawn, whatever the size of
// the array or the number of its dimensions. A member of an element reads
// as its bits in a constraint and after the call. Switching the array off
// with `rand_mode(0)` keeps an illegal value (18.8).
typedef enum bit [2:0] { A = 1, B = 3, C = 6 } e_t;
typedef enum bit [1:0] { R = 1, G = 2 } c_t;
typedef struct packed { c_t c; e_t k; bit [3:0] v; } s_t;
typedef struct packed { s_t in; bit p; } o_t;

class e_c;
  rand e_t a[4];
  rand e_t d[];
  rand e_t q[$];
  rand c_t m[2][2];
  rand s_t s;
  rand s_t sa[3];
  rand o_t o;
  constraint c {
    d.size() == 3;
    q.size() == 2;
    a[0] != A;
    d[1] == B;
    m[1][1] != R;
    foreach (sa[i]) sa[i].v == i;
    sa[2].c == G;
    o.in.c == G;
  }
endclass

module tb;
  initial begin
    static e_c e = new;
    int seen[8], kinds[8];
    int bad;
    repeat (400) begin
      if (e.randomize() != 1) $display("failed");
      foreach (e.a[i]) begin
        seen[e.a[i]]++;
        if (!(e.a[i] inside {A, B, C})) bad++;
      end
      foreach (e.d[i]) if (!(e.d[i] inside {A, B, C})) bad++;
      foreach (e.q[i]) if (!(e.q[i] inside {A, B, C})) bad++;
      foreach (e.m[i, j]) if (!(e.m[i][j] inside {R, G})) bad++;
      kinds[e.s.k]++;
      if (!(e.s.k inside {A, B, C}) || !(e.s.c inside {R, G})) bad++;
      foreach (e.sa[i]) begin
        if (!(e.sa[i].k inside {A, B, C}) || e.sa[i].v != i) bad++;
        if (!(e.sa[i].c inside {R, G})) bad++;
      end
      if (e.a[0] == A || e.d[1] != B || e.m[1][1] != G || e.sa[2].c != G) bad++;
      if (e.o.in.c != G || !(e.o.in.k inside {A, B, C})) bad++;
    end
    $display("bad=%0d", bad);
    foreach (seen[i]) if (seen[i] != 0) $display("value %0d", i);
    foreach (kinds[i]) if (kinds[i] != 0) $display("kind %0d", i);
    e.q.rand_mode(0);
    e.q.delete();
    e.q.push_back(e_t'(0));
    e.q.push_back(A);
    $display("%0d %0d", e.randomize(), e.q[0]);
    $finish(0);
  end
endmodule
