// FIN-001 G3 (F09-04): a fork-join branch that writes a process-handle
// `ref` formal of its task is legal (SV 9.3.2, 13.5.2) but not supported: a
// process formal aliases the caller's counted slot, which the branch would
// not bind, so llg rejects it instead of losing the write.
module tb;
  process h;
  task automatic t(ref process r);
    fork
      #1 r = process::self();
      #2 $display("%0d", r == null);
    join
  endtask
  initial begin
    t(h);
    $display("%0d", h == null);
    $finish;
  end
endmodule
