// A queue member of an element of a member array of 16 or more cells, which a
// subroutine record holds in its own slot, is reached only through a `ref`
// formal bound to the element; other uses reject at their location.
module tb;
  typedef struct { string s; int n; int q[$]; } el_t;
  typedef struct { el_t e[16]; int k; } rec_t;
  task automatic body();
    rec_t t;
    t.e[1].n = 77;
    t.e[1].q.push_back(4);
    $display("%0d", t.e[1].n);
  endtask
  initial body();
endmodule
