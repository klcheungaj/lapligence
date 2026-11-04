// String elements of fixed and resizable arrays as `ref`, `const ref`, `inout`
// and `output` actuals (SV 13.5.2, 7.10.3, 7.8). See the readme.
module tb;
  string fs[3];
  string fo[3];
  string fd[1:3];
  string f2[2][2];
  string qs[$];
  string ds[];
  string ai[int];
  string as[string];
  int k;

  class holder;
    string sa[2];
    string sq[$];
    string sd[];
    task touch(ref string x);
      x = {x, "m"};
      #1 x = {x, "n"};
    endtask
  endclass

  task automatic io(inout string x);
    x = {x, "!"};
  endtask
  task automatic ou(output string x);
    x = "out";
  endtask
  task automatic tr(ref string x);
    x = {x, "?"};
  endtask
  task automatic late(ref string x);
    x = "1";
    #1 x = {x, "2"};
    #1 $display("late %s", x);
  endtask
  task automatic cr(const ref string x);
    $display("cr '%s' %0d", x, x.len());
  endtask
  task automatic forward(ref string x);
    tr(x);
    late(x);
  endtask
  task automatic pair(ref string a, ref string b);
    a = "shared";
    $display("pair %s", b);
    b = "both";
    $display("pair %s", a);
  endtask
  function automatic void fr(ref string x);
    x = {x, "f"};
  endfunction
  function automatic int flen(ref string x);
    x = {x, "g"};
    return x.len();
  endfunction
  function automatic string fup(ref string x);
    x = x.toupper();
    return {x, "."};
  endfunction

  task automatic stores(ref string x);
    x = "abc";
    $display("s1 %s", qs[1]);
    x[1] = "Z";
    $display("s2 %s", qs[1]);
    $sformat(x, "%0d-%s", 7, "t");
    $display("s3 %s", qs[1]);
    x = x.substr(0, 1);
    $display("s4 %s", qs[1]);
    qs[1] = "from q";
    $display("s5 %s", x);
    x = {x, x};
    $display("s6 %s", qs[1]);
  endtask

  task automatic shifted(ref string x);
    x = "a";
    qs.push_front("zz");
    $display("B1 x=%s q0=%s q1=%s sz=%0d", x, qs[0], qs[1], qs.size());
    x = "b";
    $display("B2 x=%s q0=%s q1=%s", x, qs[0], qs[1]);
    qs.insert(1, "yy");
    x = "c";
    $display("B3 x=%s q1=%s q2=%s", x, qs[1], qs[2]);
    qs.reverse();
    x = "d";
    $display("B4 x=%s sz=%0d q0=%s q1=%s q2=%s", x, qs.size(), qs[0], qs[1], qs[2]);
    qs.sort();
    x = "e";
    $display("B5 x=%s q0=%s q1=%s q2=%s", x, qs[0], qs[1], qs[2]);
    void'(qs.pop_front());
    x = "f";
    $display("B6 x=%s sz=%0d q0=%s", x, qs.size(), qs[0]);
    qs.delete(0);
    $display("B7 x=%s sz=%0d", x, qs.size());
    x = "g";
    $display("B8 x=%s sz=%0d q0=%s", x, qs.size(), qs[0]);
    qs.delete();
    x = "h";
    qs.push_back("w");
    $display("B9 x=%s sz=%0d q0=%s", x, qs.size(), qs[0]);
  endtask

  task automatic assigned(ref string x);
    x = "y";
    qs = {"m", "n"};
    $display("C1 x=%s q=%s %s", x, qs[0], qs[1]);
    x = "k";
    $display("C2 x=%s q=%s %s", x, qs[0], qs[1]);
  endtask

  task automatic fixed_assign(ref string x);
    x = "1";
    fs = fo;
    $display("D1 x=%s f=%s", x, fs[1]);
    x = "2";
    $display("D2 x=%s f=%s", x, fs[1]);
  endtask

  task automatic assoc_ops(ref string x);
    $display("E1 before num=%0d", as.num());
    x = "v";
    $display("E2 x=%s num=%0d e=%s", x, as.num(), as["w"]);
    as.delete("w");
    x = "u";
    $display("E3 x=%s num=%0d", x, as.num());
    as["w"] = "back";
    x = "t";
    $display("E4 x=%s e=%s", x, as["w"]);
  endtask

  task automatic assoc_assign(ref string x);
    string other[int];
    other[2] = "other";
    x = "1";
    ai = other;
    $display("F1 x=%s n=%0d", x, ai.num());
    x = "2";
    $display("F2 x=%s v=%s", x, ai[2]);
  endtask

  task automatic dyn_ops(ref string x);
    x = "5";
    ds = new[4];
    $display("G1 x=%s d0='%s'", x, ds[0]);
    x = "6";
    $display("G2 x=%s d0='%s'", x, ds[0]);
  endtask

  task automatic autos();
    string s[3];
    string q[$];
    fork
      #1 $display("mid '%s'", s[1]);
    join_none
    late(s[1]);
    q = {"a", "b"};
    late(q[1]);
    $display("autos %s %s", s[1], q[1]);
  endtask

  task automatic canceled(ref string x);
    x = "start";
    #5 x = "never";
  endtask

  initial begin
    holder h = new;
    int l;
    k = 1;
    fs[1] = "a";
    qs = {"a", "b"};
    ds = new[2];
    ds[1] = "d";
    ai[3] = "e";
    f2[1][0] = "z";
    fd[2] = "r";
    io(fs[1]); io(qs[k]); io(ds[k]); io(ai[3]); io(f2[1][0]); io(ai[9]); io(fd[2]);
    $display("%s %s %s %s %s %s %s", fs[1], qs[1], ds[1], ai[3], f2[1][0], ai[9], fd[2]);
    ou(fs[0]); ou(qs[0]); ou(ds[0]); ou(ai[0]); ou(f2[0][1]); ou(as["o"]);
    $display("%s %s %s %s %s %s", fs[0], qs[0], ds[0], ai[0], f2[0][1], as["o"]);
    tr(fs[1]); tr(qs[k]); tr(ds[k]); tr(ai[3]); tr(f2[1][0]); tr(ai[10]); tr(fd[2]);
    $display("%s %s %s %s %s %s %s", fs[1], qs[1], ds[1], ai[3], f2[1][0], ai[10], fd[2]);
    cr(fs[1]); cr(qs[k]); cr(ds[k]); cr(ai[3]); cr(f2[1][0]); cr(ai[77]);
    $display("ai num %0d", ai.num());
    fr(fs[1]); fr(qs[k]); fr(ds[k]); fr(ai[3]);
    l = flen(fs[1]);
    $display("%0d %s %s %s %s", l, fs[1], qs[1], ds[1], ai[3]);
    $display("%s %s", fup(qs[k]), qs[1]);
    k = 0;
    l = 2;
    forward(fs[l]);
    $display("forward %s", fs[2]);
    pair(fs[0], fs[0]);
    $display("pair end %s", fs[0]);
    qs = {"a", "b"};
    stores(qs[1]);
    $display("stores %s", qs[1]);
    qs = {"p", "q"};
    shifted(qs[0]);
    qs = {"p", "q"};
    assigned(qs[0]);
    fo[1] = "other";
    fixed_assign(fs[1]);
    assoc_ops(as["w"]);
    ai[2] = "x";
    assoc_assign(ai[2]);
    ds = new[2];
    dyn_ops(ds[0]);
    autos();
    h.sq = {"a"};
    h.sd = new[2];
    h.touch(h.sa[1]);
    h.touch(h.sq[0]);
    h.touch(h.sd[1]);
    l = 1;
    late(h.sa[l]);
    $display("%s %s %s", h.sa[1], h.sq[0], h.sd[1]);
    qs = {"p", "q"};
    fork : cancel_me
      canceled(qs[1]);
    join_none
    #1 disable cancel_me;
    qs[1] = "after";
    qs.push_front("o");
    $display("%s %s %s", qs[0], qs[1], qs[2]);
    $finish;
  end
endmodule
