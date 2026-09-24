// Wrong-tag selected NBA writes diagnose and do not touch the inactive payload.
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
    values[0] = tagged B(8'h55);
    values[1] = tagged A(8'h22);
    values[pick_index()].A <= 8'h33;
    #1;
    if (calls !== 1 || values[0].B !== 8'h55 || values[1].A !== 8'h22)
      $fatal(1, "inactive selected NBA mutated storage");
    $display("AFTER_INACTIVE_SELECTED_NBA calls=%0d active_B=%h", calls, values[0].B);
    $finish;
  end
endmodule
