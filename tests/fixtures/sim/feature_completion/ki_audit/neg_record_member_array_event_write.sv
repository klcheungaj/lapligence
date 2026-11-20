// A whole member array of an element written through an event control: a
// located limit (docs/known_issues.md, unpacked records with dense member
// arrays).
module tb;
  typedef int pair_t[2];
  typedef struct { int a[2]; string s; } elem_t;
  typedef struct { elem_t e[2]; } m_t;
  m_t m;
  pair_t v = '{5, 6};
  event e;
  initial begin
    m.e[1].a <= @(e) v;
    #1 -> e;
    #1 $display("%0d %0d", m.e[1].a[0], m.e[1].a[1]);
    $finish(0);
  end
endmodule
