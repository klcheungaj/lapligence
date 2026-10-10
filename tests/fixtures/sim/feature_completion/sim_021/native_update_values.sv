// IEEE 1800-2009 11.11, 11.4.2: overloaded increments, decrements and
// compound assignments used as assignment values on records with string and
// queue members, and a compound value on a fixed array above the packed value
// limit. A prefix or compound form yields the updated target, a postfix form
// the value before the update; the old value is an independent copy. Each
// update calls its bound function once (`calls`).
module tb;
  typedef struct { string s; int n; int q[$]; } t_t;
  typedef int big_t[65537];
  int calls;

  function automatic t_t tadd(t_t a, int b);
    calls++;
    tadd = a;
    tadd.s = {a.s, "+"};
    tadd.n = a.n + b;
    tadd.q.push_back(b);
  endfunction
  function automatic t_t tinc(t_t a);
    calls++;
    tinc = a;
    tinc.s = {a.s, "i"};
    tinc.n = a.n + 1;
  endfunction
  function automatic t_t tdec(t_t a);
    calls++;
    tdec = a;
    tdec.s = {a.s, "d"};
    tdec.n = a.n - 1;
  endfunction
  function automatic big_t badd(big_t a, int b);
    big_t r;
    calls++;
    foreach (a[i]) r[i] = a[i] + b;
    return r;
  endfunction

  bind + function t_t tadd(t_t, int);
  bind ++ function t_t tinc(t_t);
  bind -- function t_t tdec(t_t);
  bind + function big_t badd(big_t, int);

  t_t x, y, arr[2];
  big_t bx, by;

  task automatic local_updates();
    t_t a, b;
    a.s = "L";
    a.n = 10;
    b = a++;
    $display("local %s %0d %s %0d", a.s, a.n, b.s, b.n);
    b = --a;
    $display("local %s %0d %s %0d", a.s, a.n, b.s, b.n);
  endtask

  initial begin
    x.s = "x";
    x.n = 0;
    y = x++;
    $display("post %s %0d %s %0d", x.s, x.n, y.s, y.n);
    y.s = "changed";
    $display("copy %s %s", x.s, y.s);
    y = ++x;
    $display("pre %s %0d %s %0d", x.s, x.n, y.s, y.n);
    y = (x += 5);
    $display("compound %s %0d %s %0d %0d", x.s, x.n, y.s, y.n, y.q.size());
    arr[1].s = "e";
    arr[1].n = 7;
    y = arr[1]--;
    $display("element %s %0d %s %0d", arr[1].s, arr[1].n, y.s, y.n);
    local_updates();
    bx[0] = 1;
    bx[65536] = 2;
    by = (bx += 3);
    $display("big %0d %0d %0d %0d", bx[0], bx[65536], by[0], by[65536]);
    $display("calls %0d", calls);
    $finish;
  end
endmodule
