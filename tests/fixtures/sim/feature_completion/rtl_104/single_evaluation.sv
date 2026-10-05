// IEEE 1800-2009 11.11: a compound assignment or increment built from an
// overloaded operator evaluates its target once; an index call with a side
// effect runs exactly once for the read and the write, also in a for step.
module tb;
  typedef struct { int a; logic [7:0] b; } s_t;
  typedef struct { s_t inner; int tag; } r_t;
  function automatic s_t inc(s_t x);
    inc.a = x.a + 1;
    inc.b = x.b + 2;
  endfunction
  function automatic s_t add(s_t x, s_t y);
    add.a = x.a + y.a;
    add.b = x.b + y.b;
  endfunction
  bind ++ function s_t inc(s_t);
  bind + function s_t add(s_t, s_t);
  s_t arr [4];
  s_t y, d;
  r_t recs [3];
  int k, j;
  function automatic int next();
    k++;
    return k;
  endfunction
  task automatic dump(input string label);
    $display("%s %0d/%0d %0d/%0d %0d/%0d %0d/%0d k=%0d", label, arr[0].a, arr[0].b, arr[1].a,
             arr[1].b, arr[2].a, arr[2].b, arr[3].a, arr[3].b, k);
  endtask
  initial begin
    foreach (arr[i]) begin
      arr[i].a = 10 * i;
      arr[i].b = 8'(i);
    end
    d.a = 5;
    d.b = 1;
    k = -1;
    arr[next()] += d;
    dump("compound");
    arr[next()]++;
    dump("postfix");
    ++arr[next()];
    dump("prefix");
    y = arr[next()]++;
    dump("value");
    $display("old %0d/%0d", y.a, y.b);
    k = -1;
    y = (arr[next()] += d);
    dump("assigned");
    $display("new %0d/%0d", y.a, y.b);
    k = 0;
    recs[1].inner.a = 3;
    recs[1].inner.b = 0;
    recs[1].tag = 9;
    recs[next()].inner++;
    $display("nested %0d %0d %0d k=%0d", recs[1].inner.a, recs[1].inner.b, recs[1].tag, k);
    k = -1;
    for (j = 0; j < 2; arr[next()]++) j++;
    dump("step");
    $finish(0);
  end
endmodule
