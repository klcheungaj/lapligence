// Run-time indices into arrays of records nested in native records
// (`m.e[k].s`) and indices with side effects (SV 7.2, 7.4.6, 11.5.1).
module tb;
  typedef struct { string s; int n; real r; } e_t;
  typedef struct { string name; e_t e[4]; string a[3]; } r_t;
  r_t m;
  int k;
  function automatic int next();
    k++;
    return k;
  endfunction
  function automatic string pick(r_t r, int i);
    r.e[i].s = {r.e[i].s, "+"};
    return {r.e[i].s, r.a[i - 1]};
  endfunction
  initial begin
    foreach (m.e[i]) begin
      m.e[i].s = $sformatf("e%0d", i);
      m.e[i].n = i * 10;
      m.e[i].r = i * 0.5;
    end
    m.a = '{"a0", "a1", "a2"};
    k = 1;
    $display("1 %s %0d %0d", m.e[k].s, m.e[k + 1].n, m.e[k].r == 0.5);
    m.e[k].s = "W";
    m.e[k + 2].n = 77;
    $display("2 %s %0d", m.e[1].s, m.e[3].n);
    k = 0;
    $display("3 %s k=%0d", m.a[next()], k);
    m.a[next()] = "Z";
    $display("4 %s %s k=%0d", m.a[1], m.a[2], k);
    $display("5 %s %s", pick(m, 2), m.e[2].s);
    k = 9;
    $display("6 [%s] %0d %0d", m.e[k].s, m.e[k].n, m.e[k].r == 0.0);
    $finish(0);
  end
endmodule
