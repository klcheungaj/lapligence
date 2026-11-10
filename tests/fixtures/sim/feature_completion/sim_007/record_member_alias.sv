module tb;
  typedef struct {
    int id;
    int m[$];
    int dm[];
    int bm[$:1];
    string names[$];
    int pad[`N];
  } rec_t;
  rec_t rq[$];
  rec_t ra[string];
  rec_t r;
  int calls;
  int a = 0, b = 0;
  function automatic int f();
    calls++;
    return 0;
  endfunction
  initial begin
    r.id = 1; rq.push_back(r);
    r.id = 2; rq.push_back(r);
    rq[0].m.push_back(1); rq[0].m.push_back(2); rq[0].m.push_back(3);
    rq[1].m.push_back(9);
    rq[a].m.push_back(rq[b].m.pop_front());
    $display("a %p %p", rq[0].m, rq[1].m);
    b = 1;
    rq[a].m.push_back(rq[b].m.pop_front());
    $display("b %p %p", rq[0].m, rq[1].m);
    rq[f()].m.push_back(7);
    $display("c calls %0d %p", calls, rq[0].m);
    rq[0].m[5] = 42;
    rq[0].m[9] = 43;
    $display("d %p size %0d", rq[0].m, rq[0].m.size());
    rq[1].dm = new[2];
    rq[1].dm[1] = 4;
    rq[1].bm.push_back(1); rq[1].bm.push_back(2); rq[1].bm.push_back(3);
    $display("e %p %p", rq[1].dm, rq[1].bm);
    rq[f()].names.push_back("x");
    rq[0].names.push_back("y");
    rq[0].names.insert(1, rq[0].names.pop_front());
    $display("f calls %0d %p", calls, rq[0].names);
    r = rq[0];
    r.m.push_back(100);
    $display("g %0d %p %p", r.id, r.m, rq[0].m);
    rq[1] = r;
    rq[1].m.delete(0);
    $display("h %p %p", rq[1].m, rq[1].names);
    ra["k"].m.push_back(5);
    ra["k"].m.push_back(ra["k"].m.size());
    $display("i %p size %0d", ra["k"].m, ra.size());
    rq[0].pad[2] = 3;
    rq[0].m.push_back(rq[0].pad[2]);
    $display("j %p", rq[0].m);
    $display("done");
    $finish;
  end
endmodule
