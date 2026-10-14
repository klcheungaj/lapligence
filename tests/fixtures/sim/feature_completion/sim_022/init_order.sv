// SIM-022-A01/A03: static initialization completes before any procedure,
// handles constructed by initializers are visible to every later reader, and
// no static local depends on which procedure calls its subroutine first.
package cfg;
  class Settings;
    int depth;
    string tag;
    function new(int d, string t);
      depth = d;
      tag = t;
    endfunction
  endclass
  Settings settings = new(4, "pkg");
  int doubled = settings.depth * 2;
  function automatic string describe();
    static string first = $sformatf("%s/%0d", settings.tag, doubled);
    return first;
  endfunction
endpackage

module tb;
  import cfg::*;
  class Node;
    int v;
    Node next;
    function new(int x);
      v = x;
    endfunction
  endclass
  Node head = new(1);
  int head_value = head.v;
  string seen = $sformatf("head=%0d", head.v);
  int counter = 5;
  int calls = 0;
  function automatic int bump();
    calls++;
    return calls;
  endfunction
  int first_bump = bump();

  function automatic int snapshot();
    static int at_start = counter;
    static Node held = head;
    return at_start * 100 + held.v;
  endfunction

  int from_a;
  int from_b;
  initial begin
    head = new(50);
    counter = 77;
    from_a = snapshot();
  end
  initial begin
    from_b = snapshot();
    #1;
    $display("a: snapshot=%0d", from_a);
    $display("b: snapshot=%0d", from_b);
    $display("head_value=%0d seen=%s first_bump=%0d", head_value, seen, first_bump);
    $display("describe=%s doubled=%0d", describe(), doubled);
    $finish;
  end
endmodule
