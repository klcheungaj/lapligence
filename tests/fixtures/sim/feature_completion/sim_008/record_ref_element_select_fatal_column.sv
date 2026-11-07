// A task waiting on a record `ref` formal bound to a run-time selected
// element stops the run when the index selects no element, at 16 cells as
// below 16 (`record_ref_element_select_fatal`): a record of the column form
// would otherwise give the waiter a default record that no write reaches.
// The first call selects an element, so its waiter still wakes.
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[16]; } rec_t;
  rec_t o;
  int i = 5;
  int j = 21;
  task automatic wtr(ref el_t r);
    @(r.n);
    $display("woke %0d", r.n);
  endtask
  initial begin
    $display("before");
    fork
      wtr(o.e[i]);
    join_none
    #1 o.e[5].n = 7;
    #1 $display("selected");
    wtr(o.e[j]);
    $display("after");
  end
endmodule
