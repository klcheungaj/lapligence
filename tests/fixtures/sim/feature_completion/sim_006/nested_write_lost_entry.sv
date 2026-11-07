// S07-D8: a value that deletes the entry its write goes through does not lose
// the write. The value names the element it replaces, sits under two keys or
// is a packed record; the selectors run once. An array member write reaches
// a copy-staged queue too.
module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pk_t;
  typedef struct { int a; string s; int arr[2]; } rec_t;
  typedef struct { int arr[2]; int v; } r2_t;
  rec_t ra[string][int];
  rec_t rc[string][string][int];
  pk_t pa[string][int];
  int aa[string][int];
  r2_t rq[string][$];
  r2_t e;
  int cnt;
  function automatic int del();
    ra.delete("x");
    return 7;
  endfunction
  function automatic rec_t g(rec_t in);
    rec_t r = in;
    r.a++;
    ra.delete("x");
    return r;
  endfunction
  function automatic rec_t gc(rec_t in);
    rec_t r = in;
    r.a++;
    rc.delete("x");
    return r;
  endfunction
  function automatic pk_t gk();
    pk_t p = 8'h5a;
    pa.delete("x");
    return p;
  endfunction
  function automatic string k();
    cnt++;
    return "x";
  endfunction
  function automatic int adel();
    aa.delete("x");
    return 5;
  endfunction
  initial begin
    ra["x"][1].a = 1;
    ra["x"][2].arr[1] = del();
    $display("%0d %0d", ra["x"].num(), ra["x"][2].arr[1]);
    ra.delete();
    ra["x"][1].a = 1;
    ra["x"][3] = g(ra["x"][1]);
    $display("%0d %0d", ra["x"].num(), ra["x"][3].a);
    rc["x"]["y"][1].a = 4;
    rc["x"]["y"][2] = gc(rc["x"]["y"][1]);
    $display("%0d %0d", rc["x"]["y"].num(), rc["x"]["y"][2].a);
    pa["x"][1] = 1;
    pa["x"][3] = gk();
    $display("%0d %0h", pa["x"].num(), pa["x"][3]);
    aa["x"][1] = 3;
    aa[k()][2] = aa[k()][1] + adel();
    $display("%0d %0d %0d", aa["x"].num(), aa["x"][2], cnt);
    rq["x"].push_back(e);
    rq["x"][0].arr[1] = 5;
    $display("%0d", rq["x"][0].arr[1]);
    $finish;
  end
endmodule
