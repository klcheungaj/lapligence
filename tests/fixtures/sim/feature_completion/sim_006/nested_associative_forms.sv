module tb;
  class S; int id; function new(int i); id = i; endfunction endclass
  class Db;
    S streams[string][string];
    function S get(string a, string b);
      if (streams.exists(a) && streams[a].exists(b)) return streams[a][b];
      get = new(a.len() + b.len());
      streams[a][b] = get;
    endfunction
    function int total();
      int n = 0;
      foreach (streams[a, b]) n += streams[a][b].id;
      return n;
    endfunction
    function void drop(string a, string b);
      streams[a].delete(b);
      if (streams[a].num() == 0) streams.delete(a);
    endfunction
  endclass
  typedef int ia_t[int];
  typedef int sa_t[string];
  int aa[string][int];
  real ra[int][string];
  string sv[string][int];
  int qa[$][int];
  sa_t da[];
  int cp[string][int];
  ia_t loc;
  int v;
  string key;
  function automatic int cnt(int a[int]);
    return a.num();
  endfunction
  function automatic ia_t mk(int k);
    mk[k] = k * 2;
  endfunction
  initial begin
    Db d = new;
    S h; S h2;
    h = d.get("ab", "c");
    h2 = d.get("ab", "c");
    $display("%0d %0d", h.id, h == h2);
    h = d.get("ab", "de");
    h = d.get("x", "y");
    h = null;
    $display("%0d %0d", d.total(), d.streams.num());
    d.drop("x", "y");
    $display("%0d %0d", d.total(), d.streams.num());
    $display("%0d", d.streams["zz"].num());
    $display("%0d", d.streams.exists("zz"));
    aa["p"][3] = 30;
    aa["p"][1] = 10;
    aa["q"][2] = 20;
    if (aa["p"].first(v)) $display("first %0d", v);
    if (aa["p"].next(v)) $display("next %0d", v);
    if (aa["p"].last(v)) $display("last %0d", v);
    if (aa["p"].prev(v)) $display("prev %0d", v);
    if (aa.first(key)) $display("%s", key);
    loc = aa["p"];
    loc[9] = 90;
    $display("%0d %0d %0d", loc.num(), aa["p"].num(), cnt(aa["p"]));
    aa["r"] = loc;
    $display("%0d", aa["r"][9]);
    aa["r"].delete(9);
    $display("%0d %0d", aa["r"].num(), loc.num());
    aa["r"].delete();
    $display("%0d %0d", aa["r"].num(), aa.exists("r"));
    aa["s"] = mk(4);
    $display("%0d", aa["s"][4]);
    cp = aa;
    cp["p"][3] = 31;
    $display("%0d %0d", aa["p"][3], cp["p"][3]);
    aa.delete("p");
    $display("%0d %0d", aa.num(), aa.exists("p"));
    ra[1]["a"] = 1.5;
    ra[1]["a"] += 1.0;
    $display("%f", ra[1]["a"]);
    sv["k"][4] = "hello";
    sv["k"][4] = {sv["k"][4], " w"};
    $display("%s %0d", sv["k"][4], sv["k"][4].len());
    qa.push_back(mk(1));
    qa.push_front(mk(2));
    qa.insert(1, mk(3));
    $display("%0d %0d %0d", qa.size(), qa[1][3], qa[0].num());
    loc = qa.pop_front();
    $display("%0d %0d", loc.num(), loc[2]);
    loc = qa[0];
    qa[0][5] = 5;
    qa[0] = loc;
    $display("%0d", qa[0].num());
    da = new[2];
    da[1]["t"] = 8;
    da[0] = da[1];
    da[1]["t"] = 9;
    $display("%0d %0d", da[0]["t"], da[1]["t"]);
    foreach (da[i, k]) $display("%0d %s %0d", i, k, da[i][k]);
    foreach (aa["s"][j]) aa["s"][j] += 1;
    foreach (aa[k]) $display("%s %0d", k, aa[k].num());
    foreach (aa["s"][j]) $display("%0d %0d", j, aa["s"][j]);
    foreach (aa["none"][j]) $display("never");
    $display("%0d", aa.exists("none"));
    $finish;
  end
endmodule
