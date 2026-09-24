// A selected NBA receiver and tag are captured once at assignment issue.
module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } tagged_t;
  tagged_t values[0:1];
  integer calls;
  function automatic int pick_index();
    calls = calls + 1;
    return 0;
  endfunction
  initial begin
    calls = 0;
    values[0] = tagged A(8'h11);
    values[1] = tagged A(8'h22);
    values[pick_index()].A <= 8'h33;
    #1;
    if (calls !== 1 || values[0].A !== 8'h33 || values[1].A !== 8'h22)
      $fatal(1, "selected NBA calls=%0d values=%h/%h", calls, values[0].A, values[1].A);
    $display("PASS r02_tagged_selected_write calls=%0d", calls);
    $finish;
  end
endmodule
