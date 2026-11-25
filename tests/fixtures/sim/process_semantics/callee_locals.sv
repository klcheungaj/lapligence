module tb;
  typedef struct { int a; string s; real r; } rec_t;
  typedef struct { int q[$]; int f[2]; } rec2_t;
  class C; int v; int q[$]; function new(int x); v = x; endfunction
    function int m(int n);
      int lq[$]; string s; int aa[int]; int da[]; int fa[3]; rec_t r; C o; event e;
      lq.push_back(n); lq.push_back(v); s = $sformatf("%0d", n); aa[n] = 1; da = new[n + 1]; fa[0] = n; r.a = n; r.s = s; o = this;
      q.push_back(n);
      return lq.sum() + s.len() + aa.num() + da.size() + fa[0] + r.a + r.s.len() + o.v;
    endfunction
  endclass
  function automatic int q_f(int n); int q[$]; q.push_back(n); q.push_back(1); return q.sum(); endfunction
  function automatic int d_f(int n); int d[]; d = new[n]; foreach (d[i]) d[i] = i; return d.sum(); endfunction
  function automatic int a_f(int n); int a[int]; a[n] = n; a[0] = 1; return a.num(); endfunction
  function automatic int s_f(int n); string a[string]; a["x"] = "y"; a[$sformatf("%0d", n)] = "z"; return a.num(); endfunction
  function automatic int str_f(int n); string s; s = $sformatf("%0d", n); return s.len(); endfunction
  function automatic int fa_f(int n); int f[3]; f[0] = n; f[1] = 2; return f[0] + f[1]; endfunction
  function automatic int fs_f(int n); string f[2]; f[0] = "ab"; f[1] = $sformatf("%0d", n); return f[0].len() + f[1].len(); endfunction
  function automatic int r_f(int n); rec_t r; r.a = n; r.s = "xx"; r.r = 1.5; return r.a + r.s.len() + int'(r.r); endfunction
  function automatic int r2_f(int n); rec2_t r; r.q.push_back(n); r.f[1] = n; return r.q.sum() + r.f[1]; endfunction
  function automatic int c_f(int n); C c; c = new(n); return c.m(n); endfunction
  function automatic int re_f(int n); real r; r = n; return int'(r) + 1; endfunction
  function automatic int ra_f(int n); real r[2]; r[0] = n; r[1] = 1; return int'(r[0]) + int'(r[1]); endfunction
  function automatic int qq_f(int n); int q[$][$]; q.push_back('{n}); q[0].push_back(2); return q[0].sum(); endfunction
  function automatic int aq_f(int n); return 2 + n; endfunction
  function automatic int ev_f(int n); event e; process p; semaphore s; mailbox m; s = new(1); m = new(); -> e; return n; endfunction
  function automatic void add(ref int q[$], input int n); q.push_back(n); q[0] += 1; endfunction
  function automatic int ref_f(int n); int q[$]; add(q, n); add(q, 1); return q.sum(); endfunction
  function automatic void addf(ref int f[2], input int n); f[0] = n; f[1] = n; endfunction
  function automatic int reff_f(int n); int f[2]; addf(f, n); return f[0] + f[1]; endfunction
  function automatic void adds(ref string s, input int n); s = {s, "a"}; endfunction
  function automatic int refs_f(int n); string s; adds(s, n); adds(s, n); return s.len(); endfunction
  function automatic int fac(int n); int q[$]; q.push_back(n); if (n < 2) return 1; return q[0] * fac(n - 1); endfunction
  function automatic int form(int q[$], int n); q.push_back(n); return q.sum(); endfunction
  function automatic int formd(int n); return form('{1, 2}, n); endfunction
  function automatic void outq(output int q[$], input int n); q.push_back(n); q.push_back(n); endfunction
  function automatic int out_f(int n); int q[$]; outq(q, n); return q.sum(); endfunction
  function int stf(int n); automatic int q[$]; automatic string s; q.push_back(n); s = "q"; return q.sum() + s.len(); endfunction
  typedef int iq_t[$];
  function automatic iq_t retq(int n); retq.push_back(n); retq.push_back(n + 1); endfunction
  function automatic int retq_f(int n); iq_t q; q = retq(n); return q.sum(); endfunction
  function automatic int blk(int n); for (int i = 0; i < n; i++) begin int q[$]; q.push_back(i); blk += q[0]; end endfunction
  int a1[24], a2[24], x;
  function automatic int all(int n, int k);
    case (k)
      0: return q_f(n); 1: return d_f(n); 2: return a_f(n); 3: return s_f(n); 4: return str_f(n); 5: return fa_f(n);
      6: return fs_f(n); 7: return r_f(n); 8: return r2_f(n); 9: return c_f(n); 10: return re_f(n); 11: return ra_f(n);
      12: return qq_f(n); 13: return aq_f(n); 14: return ev_f(n); 15: return ref_f(n); 16: return reff_f(n); 17: return refs_f(n);
      18: return fac(n); 19: return formd(n); 20: return out_f(n); 21: return stf(n); 22: return retq_f(n); 23: return blk(n);
    endcase
    return 0;
  endfunction
  always_comb for (int k = 0; k < 24; k++) a1[k] = all(x, k);
  always_comb for (int k = 0; k < 24; k++) a2[k] = all(x + 1, k);
  initial begin
    x = 3; #1; foreach (a1[k]) $display("%0d %0d %0d", k, a1[k], a2[k]);
    $finish;
  end
endmodule
