// Decision S13-D6 (the run stops, so the expected output ends at the last line
// printed before the call): a task that waits on a `ref` formal bound to an
// element that does not exist stops the run, instead of waiting for a change
// that no store can make. The first call below binds element 2 and its waiter
// wakes at the store; the second binds element 9 of a four-element array, which
// no index selects, and the run ends there with a run-time error.
//
// IEEE 1800-2009 13.5.2 (SystemVerilog-1800-2009.txt L19138-19141, L19143-19147):
//   "When the argument is passed by reference, both the caller and the
//   subroutine share the same representation of the argument; therefore, any
//   changes made to the argument, within either the caller or the subroutine,
//   shall be visible to each other."
//   "Only the following shall be legal to pass by reference: ... an element of
//   an unpacked array."
// 7.4.6 (L8213): "A single element of a packed or unpacked array can be
// selected using an indexed name." The text gives no storage for an element
// that does not exist, so nothing is shared and a change can never be seen.
// It does not say what binding such a reference does; llg and industry-standard
// behaviour end the run at the call when the callee waits on the formal.
module tb;
  typedef struct { int n; int m; } el_t;
  el_t o [4];
  int i = 2;
  int j = 9;
  task automatic wtr(ref el_t r);
    @(r.n);
    $display("woke %0d", r.n);
  endtask
  initial begin
    $display("before");
    fork
      wtr(o[i]);
    join_none
    #1 o[2].n = 7;
    #1 $display("selected");
    wtr(o[j]);
    $display("after the stop");
    $finish;
  end
endmodule
