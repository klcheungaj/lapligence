// A `ref` process formal bound to a class property: a located limit
// (docs/known_issues.md, rejected process-handle forms).
module tb;
  class holder;
    process p;
  endclass
  task automatic take(ref process p);
    p = process::self();
  endtask
  initial begin
    holder h = new;
    take(h.p);
    $display("%0d", h.p != null);
    $finish(0);
  end
endmodule
