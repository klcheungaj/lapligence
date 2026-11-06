module tb;
  typedef struct { int a; string s; int arr[3]; } rec_t;
  rec_t q[$];
  rec_t d[];
  rec_t m[string];
  rec_t r;
  class C; rec_t v; rec_t l[$]; endclass
  C c;
  initial begin
    c = new;
    q.push_back('{1, "x", '{0, 1, 2}});
    q[0].a += 2;
    q[0].a++;
    q[0].arr[1] += 3;
    m["k"].a += 1;
    m["k"].s = {m["k"].s, "z"};
    r.a += 1;
    c.v.a += 4;
    c.l.push_back(r);
    c.l[0].a *= 3;
    $display("%0d %0d %0d %0d", q[0].a, q[0].arr[1], m["k"].a, c.l[0].a);
    $display("%s %0d %0d", m["k"].s, r.a, c.v.a);
    q[0].a--;
    ++q[0].arr[2];
    $display("%0d %0d", q[0].a++, q[0].a);
    $display("%0d", --m["k"].a);
    $display("%0d %0d", q[0].a, q[0].arr[2]);
    $finish;
  end
endmodule
