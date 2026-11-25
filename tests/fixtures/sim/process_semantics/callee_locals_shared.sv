package lib;
  function automatic int f(int n);
    int q[$];
    q.push_back(n);
    return q.sum();
  endfunction
  function automatic int g(int n);
    string s[$];
    s.push_back("a");
    return s.size() + n;
  endfunction
endpackage

interface helper;
  function automatic int f(int n);
    int q[$];
    q.push_back(n);
    return q.sum() + 1;
  endfunction
endinterface

module sub(input int i, output int o, output int o2);
  import lib::*;
  function automatic int l(int n);
    int q[$];
    q.push_back(n);
    return q.sum() + 2;
  endfunction
  always_comb o = f(i) + g(i) + l(i);
  assign o2 = f(i) + l(i);
endmodule

module tb;
  import lib::*;
  class K;
    static function int sf(int n);
      int q[$];
      q.push_back(n);
      q.push_back(1);
      return q.sum();
    endfunction
    virtual function int vf(int n);
      string s;
      int q[$];
      s = "abc";
      q.push_back(n);
      return s.len() + q.sum();
    endfunction
  endclass

  task automatic t(input int n, output int o);
    int q[$];
    int aa[string];
    string s;
    q.push_back(n);
    aa["k"] = n;
    s = "ab";
    o = q.sum() + aa.num() + s.len();
  endtask

  int x, a, b, c, d, e, h, m1, m2, c1, c2, l1;
  helper i();
  helper j();
  K k = new;
  wire [31:0] w, w2 = f(x) + 2;

  sub s1(x, a, b);
  sub s2(x + 1, c, d);
  always_comb e = i.f(x) + f(x);
  always_comb h = i.f(x) + j.f(x);
  assign w = f(x) + i.f(x);
  always_comb t(x, m1);
  always_comb t(x + 1, m2);
  always_comb c1 = K::sf(x) + k.vf(x);
  always_comb c2 = K::sf(x + 1) + k.vf(x + 1);
  always_latch begin
    automatic int own[] = new[2];
    own[0] = x;
    l1 = own.sum();
  end

  initial begin
    x = 5;
    #1 x = 6;
    #1 $display("%0d %0d %0d %0d %0d %0d %0d %0d", a, b, c, d, e, h, w, w2);
    $display("%0d %0d %0d %0d %0d", m1, m2, c1, c2, l1);
    $finish;
  end
endmodule
