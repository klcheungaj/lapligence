// A task that does not wait on its `ref` formal still runs when the index
// selects no element: the formal reads the default value of the element type
// and the generated code carries no stop (compare `ref_wait_unselected_*`,
// where the callee waits). SV 13.5.2 and 7.4.6 do not say what a reference to
// a nonexistent element binds; llg keeps the reading of
// `record_ref_element_select`.
module tb;
  typedef struct { int n; int m; } el_t;
  typedef struct packed { logic [7:0] a; logic [7:0] b; } pk_t;
  el_t ro [4];
  pk_t po [4];
  int iq [$];
  int j = 9;
  task automatic rrec(ref el_t r);
    $display("rec %0d", r.n);
  endtask
  task automatic rpk(ref pk_t r);
    $display("pk %0d", r);
  endtask
  task automatic rint(ref int r);
    $display("int %0d", r);
  endtask
  initial begin
    iq.push_back(3);
    rrec(ro[j]);
    rpk(po[j]);
    rint(iq[j]);
    $display("done");
  end
endmodule
