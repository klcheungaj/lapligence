// Event controls and waits on members of record elements.
module tb;
  typedef struct { string s; int n; logic [7:0] v; real r; int a[2]; } rec_t;
  typedef struct { int n; logic [7:0] v; } prec_t;
  rec_t nb[3];
  rec_t q[$];
  rec_t aa[string];
  prec_t pb[3];
  string names[3];
  int cnt = 0, cnt2 = 0, cnt3 = 0, cnt4 = 0, cnt5 = 0, k = 1;
  always @(nb[1].n) begin cnt++; $display("%0d nb1.n %0d", $time, nb[1].n); end
  always @(pb[1].n) begin cnt2++; $display("%0d pb1.n %0d", $time, pb[1].n); end
  always @(posedge nb[1].v[0]) begin cnt3++; $display("%0d posedge v0", $time); end
  always @(nb[2].s) begin cnt4++; $display("%0d nb2.s %s", $time, nb[2].s); end
  always @(nb[k].n or nb[0].r) begin cnt5++; #0 $display("%0d nb[k].n %0d %0.1f", $time, nb[k].n, nb[0].r); end
  always @(names[1]) $display("%0d names1 %s", $time, names[1]);
  always @(nb[0].a[1]) $display("%0d nb0.a1 %0d", $time, nb[0].a[1]);
  initial begin
    q.push_back(nb[0]);
    aa["k"] = nb[0];
    #1 nb[1].n = 5;
    #1 nb[0].n = 7;
    #1 nb[1].n = 5;
    #1 nb[1].n = 6;
    #1 nb[1].v = 8'h01;
    #1 nb[2].s = "abc";
    #1 nb[1] = '{s:"x", n:9, v:8'h00, r:1.5, a:'{1, 2}};
    #1 nb[1].v = 8'hFF;
    #1 pb[1].n = 3;
    #1 k = 2;
    #1 nb[2].n = 4;
    #1 nb[0].r = 2.5;
    #1 names[1] = "b";
    #1 names[1] = "b";
    #1 nb[0].a[1] = 3;
    #1 q[0].n = 11;
    #1 aa["k"].s = "w";
    #1 $display("%0d %0d %0d %0d %0d", cnt, cnt2, cnt3, cnt4, cnt5);
    $finish(0);
  end
  initial begin
    wait (nb[2].n == 4) $display("%0d waited %0d", $time, nb[2].n);
    @(q[0].n) $display("%0d q0.n %0d", $time, q[0].n);
    @(aa["k"].s) $display("%0d aa.s %s", $time, aa["k"].s);
  end
endmodule
