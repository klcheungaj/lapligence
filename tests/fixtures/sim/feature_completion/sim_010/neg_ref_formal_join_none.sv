// FIN-001 G3 (F09-04): a fork-join_none branch that names a `ref` formal
// outside a block-item initializer is illegal (SV 9.3.2); the frontend only
// warns, llg rejects it with the branch location.
module tb;
  int a = 1;
  task automatic t(ref int r);
    fork
      #1 r = r + 1;
    join_none
  endtask
  initial begin
    t(a);
    #2 $display("%0d", a);
    $finish;
  end
endmodule
