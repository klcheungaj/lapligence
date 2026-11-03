// FIN-001 G3 (F09-04): a fork-join branch that writes a class-handle `ref`
// formal of its task is legal (SV 9.3.2, 13.5.2) but not supported: the
// branch would not bind the actual's change marker, so llg rejects it
// instead of losing the write.
module tb;
  class C; int v; endclass
  C h;
  task automatic t(ref C r);
    fork
      #1 r = new;
      #2 $display("%0d", r == null);
    join
  endtask
  initial begin
    t(h);
    $display("%0d", h == null);
    $finish;
  end
endmodule
