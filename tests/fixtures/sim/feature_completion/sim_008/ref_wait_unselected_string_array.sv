// A task waiting on a `ref` formal bound to an element of a fixed array of strings stops the run when the
// index selects no element (SV 13.5.2, 7.4.6): the actual shares no storage,
// so a waiter on the default would never wake. The first call selects an
// element, so its waiter still wakes. The second call, made with the same
// task from the same statement form, selects none and ends the run.
module tb;
  string o [4];
  int i = 2;
  int j = 9;
  task automatic wtr(ref string r);
    @(r);
    $display("woke %0d", r.len() + 6);
  endtask
  initial begin
    $display("before");
    fork
      wtr(o[i]);
    join_none
    #1 o[2] = "a";
    #1 $display("selected");
    wtr(o[j]);
    $display("BAD after the stop");
  end
endmodule
