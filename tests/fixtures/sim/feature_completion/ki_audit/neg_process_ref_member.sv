// A `ref` process formal bound to a record member: a located limit
// (docs/known_issues.md, rejected process-handle forms).
module tb;
  typedef struct { process p; int n; } rec_t;
  rec_t r;
  task automatic take(ref process p);
    p = process::self();
  endtask
  initial begin
    take(r.p);
    $display("%0d", r.p != null);
    $finish(0);
  end
endmodule
