// A whole record with a class handle member assigned to a member array
// element of a leaf-form record: a located limit (docs/known_issues.md).
module tb;
  class C; int v; endclass
  typedef struct { C h; int v; } e_t;
  typedef struct { int x; e_t a[8]; } rec_t;
  rec_t r;
  e_t x;
  initial begin
    x.h = new;
    x.v = 3;
    r.a[5] = x;
    $display("%0d", r.a[5].v);
    $finish;
  end
endmodule
