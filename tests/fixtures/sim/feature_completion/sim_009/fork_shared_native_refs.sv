// SIM-009-A03: a task-local string or class handle that fork branches share
// keeps a change marker once a wait can observe it, so branches waiting on
// it directly and tasks waiting on it through a `ref` formal (recursively)
// wake on every change, whichever process writes it (SV 9.4.2, 9.4.3,
// 13.5.2, 9.3.2).
module tb;
  class Item;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass

  task automatic spell(ref string s, input int depth);
    if (depth > 0) spell(s, depth - 1);
    else begin
      @(s);
      $display("spell %s at %0d", s, $time);
      wait (s == "b");
      $display("spell done at %0d", $time);
    end
  endtask

  task automatic held(ref Item h, input int n);
    wait (h != null);
    $display("held %0d %0d at %0d", n, h.v, $time);
  endtask

  task automatic owner(input int n);
    string s;
    Item h;
    fork
      spell(s, 2);
      held(h, n);
      begin
        wait (s == "c");
        $display("branch s at %0d", $time);
      end
      begin
        wait (h != null);
        @(h);
        $display("branch h %0d at %0d", h.v, $time);
      end
    join_none
    #1 s = "a";
    #1 h = new(n);
    #1 s = "b";
    #1 s = "c";
    #1 h = new(n + 1);
    #1;
  endtask

  task automatic quiet();
    string q = "x";
    fork
      #1 q = "y";
    join
    $display("quiet %s at %0d", q, $time);
  endtask

  initial begin
    owner(7);
    quiet();
    $finish;
  end
endmodule
