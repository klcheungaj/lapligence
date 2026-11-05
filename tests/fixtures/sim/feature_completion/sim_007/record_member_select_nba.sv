// Nonblocking writes below a record element through run-time indices.
module tb;
  typedef struct { string s[3]; int a[3]; logic [7:0] b; real r[2]; } el_t;
  typedef struct { string name; el_t e[3]; } rec_t;
  el_t nb[3];
  el_t zero;
  rec_t m;
  int k = 1, j = 2, n = 0;
  function automatic int bump();
    n++;
    return 1;
  endfunction
  task automatic stat();
    static rec_t r;
    r.e[k].a[j] <= 13;
    r.e[k].s[j] <= "st";
    k = 0;
    #1 $display("%0d %s", r.e[1].a[2], r.e[1].s[2]);
    k = 1;
  endtask
  initial begin
    // The position is fixed at issue.
    nb[1].a[k] <= 5;
    m.e[k].a[j] <= 7;
    m.e[k].b[2] <= 1'b1;
    m.e[k].s[j] <= "str";
    nb[k].s[j] <= "el";
    nb[k].r[k] <= 1.5;
    k = 0;
    j = 0;
    $display("%0d %0d [%s]", nb[1].a[1], m.e[1].a[2], m.e[1].s[2]);
    #1 $display("%0d %0d %b [%s] [%s] %0.1f", nb[1].a[1], m.e[1].a[2], m.e[1].b[2], m.e[1].s[2],
                nb[1].s[2], nb[1].r[1]);
    // A delayed write commits at its own time.
    k = 1;
    j = 2;
    nb[k].a[j] <= #2 11;
    m.e[k].a[j] <= #2 12;
    k = 0;
    #1 $display("%0d %0d", nb[1].a[2], m.e[1].a[2]);
    #1 $display("%0d %0d", nb[1].a[2], m.e[1].a[2]);
    // A whole-element write in between does not lose the queued write.
    k = 1;
    nb[k].a[j] <= 21;
    nb[1] = zero;
    #1 $display("%0d", nb[1].a[2]);
    // One evaluation of an index with side effects.
    nb[bump()].a[bump()] <= 31;
    #1 $display("%0d %0d", nb[1].a[1], n);
    // An index out of range writes nothing.
    k = 7;
    nb[1].a[k] <= 41;
    m.e[k].a[1] <= 42;
    m.e[1].a[k] <= 43;
    #1 $display("%0d %0d %0d", nb[1].a[0], m.e[1].a[1], m.e[1].a[0]);
    k = 1;
    j = 2;
    stat();
    $finish(0);
  end
endmodule
