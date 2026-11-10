module tb;
  int qq[$][$];
  int aa[string][int];
  typedef struct { int id; int m[$]; } rec_t;
  rec_t rq[$];
  rec_t r;
  function automatic void fill(output int o[$]);
    qq[0] = '{1};
    o = '{5, 6};
  endfunction
  function automatic int repl();
    qq[0] = '{7, 8, 9};
    return 4;
  endfunction
  function automatic int drop();
    qq.delete(0);
    return 4;
  endfunction
  function automatic void fillm(output int o[$]);
    r.id = 3;
    r.m = '{1};
    rq[0] = r;
    o = '{5, 6};
  endfunction
  function automatic void filla(output int o[int]);
    aa["x"] = '{1: 1};
    o = '{2: 5, 3: 6};
  endfunction
  initial begin
    qq.push_back({});
    qq.push_back({});
    fill(qq[0]);
    $display("a %p", qq[0]);
    qq[0].push_back(repl());
    $display("b %p", qq[0]);
    r.id = 1;
    rq.push_back(r);
    fillm(rq[0].m);
    $display("c %0d %p", rq[0].id, rq[0].m);
    aa["x"][0] = 0;
    filla(aa["x"]);
    $display("d %0d %0d %0d", aa["x"].size(), aa["x"][2], aa["x"][3]);
    qq[1].push_back(1);
    qq[0].push_back(drop());
    $display("e %0d %p", qq.size(), qq[0]);
    $display("done");
    $finish;
  end
endmodule
