// A whole record of packed members assigned to an element of a member array
// of fewer than 16 cells at a run-time index: a located limit of the per-cell
// form (docs/known_issues.md, unpacked records with dense member arrays).
module tb;
  typedef struct { int a; int b; } row_t;
  typedef struct { row_t rows[4]; string s; } tab_t;
  tab_t r;
  row_t x;
  int i;
  initial begin
    x = '{9, 8};
    i = 2;
    r.rows[i] = x;
    $display("%0d %0d", r.rows[2].a, r.rows[2].b);
    $finish(0);
  end
endmodule
