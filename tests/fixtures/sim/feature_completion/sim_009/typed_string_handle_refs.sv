// SIM-009-A03: waits and event controls on string and class-handle `ref`
// formals wait on the change marker of the variable the actual names, on the
// typed call path, so the tasks recurse (SV 9.4.2, 9.4.2.3, 9.4.3, 13.5.2).
module tb;
  class Box;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass

  string word;
  Box box, box2;
  event go;

  task automatic spell(ref string s, input int n);
    wait (s == $sformatf("w%0d", n));
    $display("spell %0d at %0d", n, $time);
    if (n < 2) spell(s, n + 1);
  endtask

  task automatic watch(ref string s, input int n);
    @(s);
    $display("watch %0d %s at %0d", n, s, $time);
    if (n > 0) watch(s, n - 1);
  endtask

  task automatic rebound(ref Box b, input int n);
    @(b);
    $display("rebound %0d %0d at %0d", n, b == null ? -1 : b.v, $time);
    if (n > 0) rebound(b, n - 1);
  endtask

  task automatic member(ref Box b, input int n);
    wait (b != null && b.v == n);
    $display("member %0d at %0d", n, $time);
    if (n < 2) member(b, n + 1);
  endtask

  task automatic qualified(ref string s, input int n);
    @(go iff s == "yes");
    $display("qualified %0d at %0d", n, $time);
  endtask

  initial begin
    automatic string local_word = "";
    fork
      spell(word, 0);
      watch(local_word, 1);
      rebound(box, 1);
      member(box2, 0);
      qualified(word, 0);
    join_none
    #1 word = "w0";
    #1 word = "w1";
    #1 word = "w2";
    #1 local_word = "a";
    #1 local_word = "a";
    #1 local_word = "b";
    #1 box = new(5);
    #1 box.v = 6;
    #1 box = null;
    #1 box2 = new(0);
    #1 box2.v = 1;
    #1 box2 = new(2);
    #1 -> go;
    #1 word = "yes";
    #1 -> go;
    #1 $finish;
  end
endmodule
