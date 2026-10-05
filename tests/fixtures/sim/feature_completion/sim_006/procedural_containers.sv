// SIM-006 A01: containers declared in procedural blocks (static by default,
// or automatic) and containers collected as module ports (SV 6.21, 7.5,
// 7.10, 23.2.2). Static block storage persists across activations;
// automatic storage restarts empty at each entry.
`timescale 1ns/1ns
module child(input int qin[$], output string sout[$]);
  always @* begin
    sout = {};
    foreach (qin[i]) sout.push_back($sformatf("v%0d", qin[i]));
  end
endmodule
module tb;
  int q[$];
  string s[$];
  child c(.qin(q), .sout(s));

  task automatic visit(input int n, output int seen, output int fresh);
    static int hist[$];
    int now[$];
    hist.push_back(n);
    now.push_back(n);
    seen = hist.size();
    fresh = now.size();
  endtask

  initial begin
    int local_q[$];
    string names[$];
    real vals[] = '{1.5, 2.5};
    int seen, fresh;
    local_q.push_back(3);
    names.push_back("x");
    names.push_front("w");
    $display("block %0d %0d %s %s %.1f", local_q.size(), local_q[0], names[0], names[1],
             vals[1]);
    for (int i = 0; i < 3; i++) begin
      automatic int round[$];
      round.push_back(i);
      if (i == 2) $display("round %0d %0d", round.size(), round[0]);
    end
    visit(5, seen, fresh);
    visit(6, seen, fresh);
    $display("visit %0d %0d", seen, fresh);
    q = '{1, 2};
    #1 $display("port %0d %s %s", s.size(), s[0], s[1]);
    q.push_back(3);
    #1 $display("port %0d %s", s.size(), s[2]);
    $finish(0);
  end
endmodule
