typedef struct { integer s; int q; } rec_t;
typedef struct { rec_t in; byte b; } outer_t;

class C;
  rec_t st;
endclass

module tb;
  rec_t st;
  rec_t arr[2];
  outer_t o;
  int r1, r2, r3, r4, r5, r6;

  task automatic loc();
    rec_t l;
    l.s = 9;
    r6 = $random(l.s);
    $display("l %0d %0d", r6, l.s);
  endtask

  initial begin
    C c = new;
    st.s = 5;
    st.q = 77;
    r1 = $random(st.s);
    $display("a %0d %0d %0d", r1, st.s, st.q);
    r1 = $random(st.s);
    $display("a %0d %0d %0d", r1, st.s, st.q);
    arr[1].s = 6;
    r2 = $random(arr[1].s);
    $display("b %0d %0d", r2, arr[1].s);
    o.in.s = 7;
    o.b = 1;
    r3 = $random(o.in.s);
    $display("c %0d %0d %0d", r3, o.in.s, o.b);
    c.st.s = 8;
    r4 = $random(c.st.s);
    $display("d %0d %0d", r4, c.st.s);
    st.s = 11;
    r5 = $dist_uniform(st.s, 0, 100);
    $display("e %0d %0d", r5, st.s);
    r5 = $dist_exponential(st.q, 10);
    $display("f %0d %0d", r5, st.q);
    loc();
    $finish;
  end
endmodule
