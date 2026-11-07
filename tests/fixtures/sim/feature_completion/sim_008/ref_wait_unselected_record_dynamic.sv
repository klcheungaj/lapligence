// A task waiting on a `ref` formal bound to an element of a dynamic array of records stops the run when the
// index selects no element (SV 13.5.2, 7.4.6): the actual shares no storage,
// so a waiter on the default would never wake. The first call selects an
// element, so its waiter still wakes. The second call, made with the same
// task from the same statement form, selects none and ends the run.
module tb;
  typedef struct { string s; int n; } el_t;
  el_t d [];
  int i = 1;
  int j = 9;
  task automatic wtr(ref el_t r);
    @(r.n);
    $display("woke %0d", r.n);
  endtask
  initial begin
    d = new[3];
    $display("before");
    fork
      wtr(d[i]);
    join_none
    #1 d[1].n = 7;
    #1 $display("selected");
    wtr(d[j]);
    $display("BAD after the stop");
  end
endmodule
