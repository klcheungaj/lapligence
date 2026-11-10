module tb;
  int sq[$][$];
  int calls = 0;
  int a = 0, b = 0;
  function automatic int f();
    calls++;
    return 0;
  endfunction
  initial begin
    sq.push_back({});
    sq.push_back({});
    sq[0].push_back(1); sq[0].push_back(2); sq[0].push_back(3);
    sq[1].push_back(9);
    sq[a].push_back(sq[b].pop_front());
    $display("same selector: sq[0] %p sq[1] %p", sq[0], sq[1]);
    b = 1;
    sq[a].push_back(sq[b].pop_front());
    $display("different selectors: sq[0] %p sq[1] %p", sq[0], sq[1]);
    sq[f()].push_back(7);
    $display("side-effect selector: calls %0d sq[0] %p", calls, sq[0]);
    sq[f()][1] = 42;
    $display("side-effect selector write: calls %0d sq[0] %p", calls, sq[0]);
    $display("done");
    $finish;
  end
endmodule
