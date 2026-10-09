// SIM-024: `%m` names the module instance, subroutine or named block that
// calls the task; `%l` the library binding (SV 21.2.1.6, 21.2.1.2).
module leaf #(parameter int D = 1);
  task show;
    $display("task %m");
  endtask
  initial begin : blk
    #(D);
    $display("leaf %m %l");
  end
endmodule
module tb;
  leaf #(1) u1 ();
  leaf #(2) u2 ();
  function automatic string where_am_i();
    return $sformatf("%m");
  endfunction
  initial begin : main
    $display("main %m");
    $display("fn %s", where_am_i());
    u1.show();
    #3 $finish(0);
  end
  for (genvar g = 0; g < 1; g++) begin : gen
    initial #(2.5) $display("gen %m %s", $sformatf("<%m>"));
  end
endmodule
