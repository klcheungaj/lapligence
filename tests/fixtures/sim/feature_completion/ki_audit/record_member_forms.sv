// Record forms that docs/known_issues.md listed as limits and llg serves: a
// nested column record assigned to a member of a record with a native member
// array, a member array of 16 cells selected on a call result, a whole
// native record written to an element of a member array at a run-time index,
// a whole record read from a member array of a queue element, and two
// continuous assignments to separate bits of one member of an element.
module tb;
  typedef struct { int a[16]; real m; } inner_t;
  typedef struct { inner_t i; string d[2]; } outer_t;
  typedef struct { int a[16]; string s; } wide_t;
  typedef struct { int a; string s; } row_t;
  typedef struct { row_t rows[4]; int n; } tab_t;
  typedef struct { int a; int b; } pr_t;
  typedef struct { pr_t e[2]; string s; } o_t;
  typedef struct { logic [3:0] a; string s; } rr_t;
  outer_t o;
  inner_t x;
  tab_t r;
  row_t row;
  o_t oq[$];
  pr_t pr;
  rr_t arr[2];
  logic one = 1'b1, zero = 1'b0;
  int i;
  function automatic wide_t f();
    foreach (f.a[k]) f.a[k] = k + 1;
    f.s = "f";
  endfunction
  assign arr[1].a[0] = one;
  assign arr[1].a[1] = zero;
  initial begin
    o_t t;
    x.a[3] = 7;
    x.m = 1.5;
    o.i = x;
    $display("%0d %g", o.i.a[3], o.i.m);
    $display("%0d", f().a[1]);
    row = '{9, "nine"};
    i = 2;
    r.rows[i] = row;
    $display("%0d %s", r.rows[2].a, r.rows[2].s);
    t.e[1] = '{3, 4};
    oq.push_back(t);
    pr = oq[0].e[1];
    $display("%0d %0d", pr.a, pr.b);
    #1 $display("%b", arr[1].a[1:0]);
    $finish(0);
  end
endmodule
