// A cell of a member array of a record selected at run time, in both record
// storage forms (built with -D N=3, one leaf per cell, and -D N=16, one
// column): a record element of a queue, a record nested in one, a static
// subroutine variable and a class property, read, written, incremented and
// waited on. An index outside the member array reads the default and its
// write does nothing. IEEE 1800-2009 7.2 (L7649-7650) and 7.4.5 (L8122);
// 7.4.6 (L8234-8238); 9.4.2 (L11807).
`ifndef N
`define N 3
`endif
module tb;
  typedef struct { int arr[`N]; string s; } rec_t;
  typedef struct { rec_t e[2]; int n; } outer_t;
  class holder;
    rec_t r;
  endclass
  rec_t q[$];
  outer_t oq[$];
  holder h;
  int k = 1, j = 2, far = `N + 3, x;
  task static waiter;
    static rec_t r;
    fork
      begin wait (r.arr[k] == 5); $display("static woke at %0d %0d", $time, r.arr[k]); end
      begin #3 r.arr[1] = 5; end
    join
  endtask
  initial begin
    rec_t r0;
    outer_t o0;
    q.push_back(r0);
    q[0].arr[j] = 7; q[0].arr[j]++; q[0].arr[k] += 4;
    x = q[0].arr[j];
    $display("q %0d %0d %0d", x, q[0].arr[1], q[0].arr[0]);
    x = q[0].arr[far]; q[0].arr[far] = 9;
    $display("far %0d %0d", x, q[0].arr[j]);
    oq.push_back(o0);
    oq[0].e[k].arr[j] = 6; oq[0].e[k].s = "s";
    $display("oq %0d %0d %s", oq[0].e[1].arr[j], oq[0].e[0].arr[j], oq[0].e[k].s);
    h = new;
    h.r.arr[j] = 3;
    $display("h %0d", h.r.arr[2]);
    fork
      begin wait (h.r.arr[k] == 5); $display("class woke at %0d %0d", $time, h.r.arr[k]); end
      begin #2 h.r.arr[k] = 5; end
      begin wait (q[0].arr[0] == 1); $display("element woke at %0d", $time); end
      begin #4 q[0].arr[0] = 1; end
    join
    waiter();
    $finish;
  end
endmodule
