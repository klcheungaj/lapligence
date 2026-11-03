// FIN-001 G3 (F09-04): a fork-join branch that writes a packed `ref` formal
// of its task is legal (SV 9.3.2, 13.5.2) but not supported: the branch
// frame would hold a copy, so llg rejects it instead of losing the write.
module tb;
  int a = 1;
  task automatic t(ref int r);
    fork
      #1 r = r + 10;
      #2 $display("%0d", r);
    join
  endtask
  initial begin
    t(a);
    $display("%0d", a);
    $finish;
  end
endmodule
