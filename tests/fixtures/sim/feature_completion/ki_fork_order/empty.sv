// A fork without statements leaves nothing to wait for (readme: "Empty").
module tb;
  task automatic show(string s);
    $display("%s", s);
  endtask
  initial begin
    fork
    join
    show("join");
    fork
    join_any
    show("join_any");
    fork
    join_none
    show("join_none");
    wait fork;
    show("wait fork");
    fork : named
      int unused;
    join
    fork
      show("child");
    join_none
    fork
    join
    show("before child");
    wait fork;
    show("end");
    disable fork;
    $finish(0);
  end
endmodule
