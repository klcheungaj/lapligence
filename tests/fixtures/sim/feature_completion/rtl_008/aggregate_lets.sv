// SV2009 11.13: a let is a template. Free names bind in the let's declaration
// scope (a caller's local of the same name is not seen); actuals bind
// positionally or by name, omitted ones take their declared default (itself
// bound in the declaration scope); typed formals convert their actuals; each
// use re-evaluates the substituted operands, so an effectful actual runs once
// per occurrence and an unselected conditional operand not at all. Packed and
// unpacked fixed aggregate results feed assignments, continuous assignments,
// comparisons, selections and declaration initializers.
package let_pkg;
  int scale = 3;
  int base = 7;
  let scaled(x) = x * scale;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } nib_t;
  typedef struct { int a; int b; } pair_t;
  let mknib(a, b = 4'hf) = nib_t'{a, b};
  let mkpair(x, y = base) = pair_t'{x, y};
  let tr(byte x) = x;
endpackage

module tb;
  import let_pkg::*;
  typedef struct packed { logic [7:0] hi; logic [7:0] lo; } word_t;
  typedef struct { int a; logic [7:0] b; } rec_t;
  typedef logic [7:0] row_t [0:3];
  typedef int arr_t [0:2];
  int base = 5;
  let swap(p) = word_t'{p.lo, p.hi};
  let mk(x, y = 3) = rec_t'{x + base, 8'(y)};
  let rev(v) = arr_t'{v[2], v[1], v[0]};
  let pick(r) = r.b;
  let both(rec_t r1, rec_t r2) = rec_t'{r1.a + r2.a, r1.b ^ r2.b};
  let rot(row_t v) = row_t'{v[1], v[2], v[3], v[0]};
  let inner(x) = x + 1;
  let outer(x) = inner(x) * 2;
  let dbl(e) = e + e;
  let choose(c, t, f) = c ? t : f;
  let wide(x) = {x, x};
  int cnt = 0;
  function automatic int nxt();
    cnt++;
    return cnt;
  endfunction
  word_t pp = '{8'h12, 8'h34};
  word_t swapped_init = swap(pp);
  rec_t ra = '{1, 8'h0f}, rb = '{2, 8'hf0}, rc, made;
  row_t rw = '{8'h1, 8'h2, 8'h3, 8'h4}, rr;
  arr_t src = '{1, 2, 3}, reversed;
  int init_from_let = scaled(4);
  pair_t pair_init = mkpair(2);
  logic [7:0] cw;
  logic [3:0] nibble = 4'h9;
  int r1, r2;
  assign cw = pick(rc);
  initial begin
    automatic int scale = 100;
    rc = both(ra, rb);
    rr = rot(rw);
    made = mk(1);
    reversed = rev(src);
    #1;
    $display("let %0d %h %h %0d %0d %0d", rc.a, rc.b, cw, init_from_let, pair_init.a, pair_init.b);
    $display("row %h %h %h %h", rr[0], rr[1], rr[2], rr[3]);
    $display("swap %h %h", swap(pp), swapped_init);
    $display("make %0d %0d %0d %0d %0d", made.a, made.b, reversed[0], reversed[1], reversed[2]);
    made = mk(.y(9), .x(2));
    $display("named %0d %0d %b", made.a, made.b, rev(src) == reversed);
    $display("pkg %h %h %0d %0d %0d", mknib(4'h1), mknib(.b(4'h2), .a(4'h3)), scaled(2), outer(5), tr(300));
    r1 = dbl(nxt());
    r2 = choose(1, nxt(), nxt());
    $display("template %0d %0d %0d %h %0d", r1, r2, cnt, wide(nibble), $bits(wide(nibble)));
    $finish(0);
  end
endmodule
