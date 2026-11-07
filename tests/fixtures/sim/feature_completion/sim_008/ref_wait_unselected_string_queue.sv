// A task waiting on a `ref` formal bound to an element of a queue of strings stops the run when the
// index selects no element (SV 13.5.2, 7.4.6): the actual shares no storage,
// so a waiter on the default would never wake. The first call selects an
// element, so its waiter still wakes. The second call, made with the same
// task from the same statement form, selects none and ends the run.
module tb;
  string q [$];
  int i = 1;
  int j = 9;
  task automatic wtr(ref string r);
    @(r);
    $display("woke %0d", r.len() + 6);
  endtask
  initial begin
    q.push_back("");
    q.push_back("");
    $display("before");
    fork
      wtr(q[i]);
    join_none
    #1 q[1] = "a";
    #1 $display("selected");
    wtr(q[j]);
    $display("BAD after the stop");
  end
endmodule
