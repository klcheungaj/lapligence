// `$strobe` and `$fstrobe` read a container actual at the end of the slot.
// 21.2.2: "The action shall occur just before simulation time is advanced and
// after all other events at that time have occurred so that the data written
// are sure to be the correct data for that simulation time."
module tb;
  int q[$]; int d[]; int arr[4] = '{10,20,100,200}; int k = 1;
  int aa[string];
  typedef struct { int n; string s; } rec_t;
  rec_t rq[$];
  string sq[$];
  function automatic int sumq(int x[$]); int t = 0; foreach (x[i]) t += x[i]; return t; endfunction
  function automatic int sumd(int x[]); int t = 0; foreach (x[i]) t += x[i]; return t; endfunction
  function automatic int size_of(int x[$]); return x.size(); endfunction
  function automatic int first(int x[$], int w); return x[0] * w; endfunction
  function automatic int two(int x[$], int y[]); return sumq(x) + sumd(y); endfunction
  function automatic int count(int x[string]); return x.num(); endfunction
  function automatic int recs(rec_t x[$]); int t = 0; foreach (x[i]) t += x[i].n; return t; endfunction
  function automatic int strs(string x[$]); return x.size(); endfunction
  function automatic int edit(int x[$]); int t = x[0]; t += 1; return t; endfunction
  initial begin
    q = {1,2}; d = new[2]; d[0] = 3; d[1] = 4;
    $strobe("sumq %0d", sumq(q));
    q.push_back(5);
    #1;
    $strobe("sumd %0d", sumd(d));
    d[0] = 30;
    #1;
    $strobe("size %0d first %0d", size_of(q), first(q, k));
    q.delete(0); k = 3;
    #1;
    $strobe("two %0d", two(q, d));
    q[0] = 7; d[1] = 40;
    #1;
    $strobe("count %0d", count(aa));
    aa["x"] = 1;
    #1;
    $strobe("recs %0d", recs(rq));
    rq.push_back('{n: 9, s: "n"});
    #1;
    $strobe("strs %0d", strs(sq));
    sq.push_back("z");
    #1;
    $strobe("edit %0d", edit(q));
    q[0] = 70;
    #1;
    $fstrobe(32'h8000_0001, "file %0d", sumq(q));
    q[1] = 1;
    #1;
    $strobe("lit %0d", sumq('{1, 2, k}));
    k = 4;
    #1;
    q = {5};
    $strobe("two %0d", two('{1, 2}, d));
    d[0] = 1;
    #1;
    #1 $finish(0);
  end
endmodule
