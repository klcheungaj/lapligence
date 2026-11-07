// A class container selected through a computed handle (SV 8.4, 7.4.5, 7.10.2):
// element selects, method calls, and subroutine results as receivers.
module tb;
  typedef struct { int a; string s; int q[$]; } rec_t;
  typedef struct { int a; string s; } rec2_t;
  class S; int z[$]; endclass
  class C;
    int q[$];
    string names[$];
    real rs[$];
    int fix[3];
    int m[string];
    rec2_t r;
    rec_t rl[$];
    S sub[2];
    C self;
    function new(); sub[0] = new; sub[1] = new; self = this; endfunction
    function C me(); return this; endfunction
    task bump(); q.push_back(q.size()); endtask
  endclass
  C list[$];
  C am[int];
  int calls;
  int x;
  function automatic C pick(int i);
    calls++;
    return list[i];
  endfunction
  function automatic int total(int i);
    int n = list[i].q.size();
    return n + list[i].q.sum();
  endfunction
  initial begin
    automatic int k = 1;
    begin automatic C t = new; list.push_back(t); end
    begin automatic C t = new; list.push_back(t); end
    am[3] = list[1];
    begin
      automatic int n = list[0].q.size();
      $display("n=%0d", n);
    end
    list[0].q.push_back(4);
    list[k].q.push_back(5);
    list[k].names.push_back("nn");
    list[k].rs.push_back(1.5);
    $display("%s %f", list[k].names[0], list[k].rs[0]);
    $display("%0d", total(1));
    // short circuit: the right operand must not run for an empty list index
    if (k > 5 && list[k + 7].q.size() > 0) $display("bad");
    if (k < 2 && list[k].q.size() > 0) $display("good");
    $display("%0d", (k > 5) ? pick(0).q.size() : 7);
    $display("calls=%0d", calls);
    $display("%0d", (k < 5) ? pick(0).q.size() : 7);
    $display("calls=%0d", calls);
    if (k > 5 || pick(1).q.size() == 1) $display("or");
    $display("calls=%0d", calls);
    for (int i = 0; i < list[1].q.size(); i++) $display("i=%0d", i);
    do k--; while (k > 0 && list[k].q.size() < 9);
    $display("k=%0d", k);
    case (list[1].q.size())
      0: $display("zero");
      1: $display("one");
    endcase
    list[1].fix[2] = 6;

    $display("%p", list[1].fix);
    #1 $display("%p", list[1].fix);
    list[1].m["a"] = 2;
    if (am[3].m.exists("a")) $display("am %0d", am[3].m["a"]);
    am[3].r.a = 3;
    am[3].r.s = "rs";
    $display("%0d %s", list[1].r.a, list[1].r.s);
    begin automatic rec_t t; t.q.push_back(9); am[3].rl.push_back(t); end
    am[3].rl[0].a = 4;
    am[3].rl[0].q.push_back(11);
    $display("%0d %0d %p", list[1].rl[0].a, list[1].rl.size(), list[1].rl[0].q);
    list[1].sub[1].z.push_back(1);
    list[1].sub[1].z.push_back(list[1].sub[1].z.size());
    $display("%p", list[1].sub[1].z);
    list[1].self.me().self.q.push_back(12);
    $display("%p", list[1].q);
    list[1].bump();
    begin automatic int rr[$]; rr = list[1].q.find with (item > 5); $display("%p", rr); rr = list[1].q.find_first_index(v) with (v == list[0].q[0] + 1); $display("%p", rr); end
    foreach (am[3].m[s]) $display("%s", s);
    foreach (pick(1).q[j]) x += pick(1).q[j];
    $display("%0d %0d", x, calls);
    foreach (list[i]) foreach (list[i].q[j]) $display("%0d %0d %0d", i, j, list[i].q[j]);
    $finish;
  end
endmodule
