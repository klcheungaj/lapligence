// A task waiting on a native record `ref` formal has no body without a binding,
// so an index that selects no element stops the run where SV 7.4.6 would give
// the callee a default record.
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[2]; } rec_t;
  rec_t o;
  int i = 5;
  task automatic wtr(ref el_t r);
    @(r.n);
  endtask
  initial begin
    $display("before");
    wtr(o.e[i]);
    $display("after");
  end
endmodule
