// A deferred action passing a named event by value: the action frame captures
// no event identity, a located limit (docs/known_issues.md, deferred
// assertion actions with container, event or virtual-interface arguments).
module tb;
  event e;
  task automatic fire(input event x);
    ->x;
  endtask
  initial begin
    assert #0 (0) else fire(e);
    #1 $finish;
  end
endmodule
