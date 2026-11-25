// SV 11.4.13: set items that are nested containers, elements of them, call results,
// members of records, and arrays of reals and handles.
`timescale 1ns/1ns
module tb;
  typedef int iq_t[$];
  typedef string sq_t[$];
  int qq[$][$], dd[][], aq[string][$], qa[$][2], calls;
  string sqq[$][$], sqa[$][2], saq[string][string];
  real rqq[$][$];
  class C; int v; endclass
  C cqq[$][$], c, d;
  int three[$][$][$];
  function automatic iq_t g(int b); calls++; return {b, b + 1}; endfunction
  function automatic sq_t gs(); return {"ga", "gb"}; endfunction
  function automatic string nm(); calls++; return "b"; endfunction
  typedef struct { string n[2]; string q[$]; int iq[$]; int f[2]; real r[2]; } rec;
  rec rq[$], rf[2];
  string aq2[$];
  string em[2][2];
  function automatic bit nat(string s, int i);
    rec r; rec l[2];
    r.n[1] = "n1"; r.q.push_back("nq"); r.iq.push_back(5); r.f[0] = 6; r.r[1] = 1.5;
    l[1].n[0] = "ln"; l[1].q.push_back("lq");
    return (s inside {r.n, r.q, l[1].n, l[1].q}) || (i inside {r.iq, r.f});
  endfunction
  int idx;
  bit hit;
  event e1, e2, eq[$];
  process pr, pq[$];
  mailbox #(int) mb, mb2, mq[$];
  semaphore sm, smq[$];
  chandle hc, hq[$];
  class K extends C; endclass
  K dk, kq[$];
  C cq[$], cd[], ca[string], cf[2], cm[2][2], n;
  real xq[$], xd[], xa[int], xf[2] = '{1.5, 2.5}, xm[2][2] = '{'{1.0, 2.0}, '{3.0, 4.0}};
  shortreal shq[$];
  parameter real PR[2] = '{1.25, 2.25};
  int iq[$];
  real r;
  int nx = 3;
  task handles();
    dk = new; cq.push_back(n); cq.push_back(c);
    cd = new[2]; cd[1] = c; ca["z"] = c; cf[1] = c; cm[1][0] = c; kq.push_back(dk);
    $display("cq %b %b %b", c inside {cq}, n inside {cq}, dk inside {cq});
    $display("cd %b %b", c inside {cd}, dk inside {cd});
    $display("ca %b %b", c inside {ca}, n inside {ca});
    $display("cf %b %b", c inside {cf}, dk inside {cf});
    $display("cm %b %b %b", c inside {cm}, dk inside {cm}, c inside {cm[0]});
    $display("kq %b %b", dk inside {kq}, c inside {kq});
    $display("null %b %b", c inside {null, cq}, n inside {null});
    hq.push_back(hc);
    $display("chandle %b", hc inside {hq});
    mb = new; mq.push_back(mb); sm = new; smq.push_back(sm);
    pr = process::self(); pq.push_back(pr); eq.push_back(e1);
    $display("event %b %b %b %b", e1 inside {e1}, e1 inside {e2}, e1 inside {eq}, e2 inside {eq});
    $display("process %b %b", pr inside {pr}, pr inside {pq});
    $display("mailbox %b %b %b %b", mb inside {mb}, mb inside {mb2}, mb inside {mq}, mb2 inside {mq});
    $display("semaphore %b %b", sm inside {sm}, sm inside {smq});
  endtask
  task reals();
    xq.push_back(1.5); xq.push_back(3.0); xd = new[2]; xd[1] = 7.5; xa[3] = 8.25;
    shq.push_back(0.5); iq.push_back(3); r = 3.0;
    $display("xq %b %b", r inside {xq}, 2.0 inside {xq});
    $display("xd %b %b", 7.5 inside {xd}, 2.0 inside {xd});
    $display("xa %b %b", 8.25 inside {xa}, 2.0 inside {xa});
    $display("xf %b %b", 2.5 inside {xf}, 2.0 inside {xf});
    $display("xm %b %b", 3.0 inside {xm}, 2.25 inside {xm});
    $display("PR %b %b", 2.25 inside {PR}, 2.0 inside {PR});
    $display("shq %b %b", 0.5 inside {shq}, 2.0 inside {shq});
    $display("iq %b %b", r inside {iq}, 2.0 inside {iq});
    $display("int %b %b", nx inside {xq}, 4 inside {xq});
  endtask
  logic [3:0] lq[$][$];
  initial begin
    c = new; d = new;
    qq.push_back('{1, 2}); qq.push_back('{3});
    dd = new[2]; dd[1] = new[2]; dd[1][1] = 9;
    aq["k"].push_back(11);
    qa.push_back('{21, 22});
    sqq.push_back('{"a", "b"}); sqa.push_back('{"c", "d"}); saq["x"]["y"] = "e";
    rqq.push_back('{1.5});
    cqq.push_back('{c});
    $display("wh %b %b %b %b", 3 inside {qq}, 9 inside {dd}, 11 inside {aq}, 22 inside {qa});
    $display("whz %b %b %b %b", 8 inside {qq}, 8 inside {dd}, 8 inside {aq}, 8 inside {qa});
    $display("str %b %b %b", "b" inside {sqq}, "d" inside {sqa}, "e" inside {saq});
    $display("strz %b %b %b", "q" inside {sqq}, "q" inside {sqa}, "q" inside {saq});
    $display("str1 %b %b %b", "b" inside {sqq[0]}, "d" inside {sqa[0]}, "e" inside {saq["x"]});
    $display("real %b %b %b", 1.5 inside {rqq}, 1.5 inside {rqq[0]}, 2.5 inside {rqq});
    $display("cls %b %b %b", c inside {cqq}, c inside {cqq[0]}, d inside {cqq});
    $display("inv %b %b %b", 1 inside {qq[7]}, 1 inside {aq["no"]}, "" inside {sqq[9]});
    $display("num %0d", aq.num());
    calls = 0;
    hit = nm() inside {"a", gs(), sqq};
    $display("selector %b %0d", hit, calls);
    rq.push_back(rf[0]);
    rq[0].n[1] = "rn"; rq[0].q.push_back("rqs"); rq[0].iq.push_back(4); rq[0].f[1] = 44;
    $display("rq %b %b %b %b", "rn" inside {rq[0].n}, "rqs" inside {rq[0].q}, 4 inside {rq[0].iq}, 44 inside {rq[0].f});
    rf[1].n[1] = "fn"; rf[1].q.push_back("fq");
    $display("rf %b %b %b", "fn" inside {rf[1].n}, "fq" inside {rf[1].q}, "fq" inside {rf[0].q});
    $display("nat %b %b %b %b %b %b %b", nat("n1", 0), nat("nq", 1), nat("ln", 1), nat("lq", 1), nat("zz", 5), nat("zz", 6), nat("zz", 1));
    aq2 = {"b", "c"};
    em[1] = '{"x", "y"};
    idx = 1;
    $display("row %b %b", "y" inside {em[idx]}, "y" inside {em[idx - 1]});
    idx = 7;
    $display("badrow %b %b", "y" inside {em[idx]}, "" inside {em[idx]});
    $display("slice %b", "b" inside {aq2[0:0]});
    $display("cond %b", "b" inside {idx == 7 ? aq2 : gs()});
    while (idx inside {g(5), 7}) idx++;
    $display("idx %0d", idx);
    for (int i = 0; i inside {qq[0], 0}; i++) $display("i %0d", i);
    $strobe("strobe %b", "b" inside {sqq[0]});
    handles();
    reals();
    lq.push_back('{4'b1x00, 4'b0011});
    $display("wild %b %b %b", 4'b1100 inside {lq[0]}, 4'b0000 inside {lq[0]}, 4'b1x00 inside {lq});
    begin int t2[$][$]; t2.push_back('{31, 32}); three.push_back(t2); end
    $display("three %b %b %b %b", 32 inside {three}, 32 inside {three[0]}, 32 inside {three[0][0]}, 33 inside {three});
    $display("methods %b %b", 2 inside {qq[0].find with (item > 1)}, 1 inside {qq[0].find with (item > 1)});
    case (3) inside
      qq[0]: $display("case element");
      g(3): $display("case call");
      default: $display("case none");
    endcase
    #1 $finish(0);
  end
endmodule
