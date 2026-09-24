// Ref reads and writes share one checked tagged-member view and captured index.
module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } tagged_t;
  tagged_t values[0:1];
  integer calls;
  function automatic int pick_index();
    calls = calls + 1;
    return 0;
  endfunction
  task automatic bump(ref tagged_t data);
    data.A = data.A + 8'h01;
  endtask
  initial begin
    calls = 0;
    values[0] = tagged A(8'h11);
    values[1] = tagged A(8'h22);
    bump(values[pick_index()]);
    if (calls !== 1 || values[0].A !== 8'h12 || values[1].A !== 8'h22)
      $fatal(1, "tagged ref selector calls=%0d", calls);
    $display("PASS r01_tagged_ref_active calls=%0d", calls);
    $finish;
  end
endmodule
