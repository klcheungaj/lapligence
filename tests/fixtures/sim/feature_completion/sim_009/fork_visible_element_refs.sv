// SIM-009-A03: an element of a fork-visible automatic array is a legal `ref`
// actual (SV 13.5.2); the task writes the parent's element and a task
// waiting on another element through its `ref` formal wakes on a branch's
// write (SV 9.4.3, 9.3.2).
module tb;
  task automatic store(ref int r, input int v);
    #1 r = v;
  endtask

  task automatic await(ref int r, input int v);
    wait (r == v);
    $display("await %0d at %0d", v, $time);
  endtask

  task automatic owner();
    int la[3];
    fork
      #3 la[2] = 9;
    join_none
    fork
      await(la[2], 9);
      store(la[0], 4);
    join
    $display("owner %0d %0d %0d at %0d", la[0], la[1], la[2], $time);
  endtask

  initial begin
    automatic int ia[2];
    fork
      #5 ia[1] = 4;
    join_none
    store(ia[0], 3);
    #5 $display("ia %0d %0d at %0d", ia[0], ia[1], $time);
    owner();
    $finish;
  end
endmodule
