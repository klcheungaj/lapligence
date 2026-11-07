// The same stop for a column-form element below a nested record with a
// descending bound, a container member and an index under the lower bound.
module tb;
  typedef struct { string s; int n; int q[$]; } el_t;
  typedef struct { el_t e[16:1]; } in_t;
  typedef struct { in_t p; } rec_t;
  rec_t o;
  int i = 0;
  task automatic wtr(ref el_t r);
    @(r.n);
  endtask
  initial begin
    $display("before");
    wtr(o.p.e[i]);
    $display("after");
  end
endmodule
