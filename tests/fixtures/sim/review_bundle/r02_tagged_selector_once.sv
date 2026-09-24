// The one syntactic selector call is not an assignment-pattern side effect.
module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } tagged_t;
  tagged_t values[0:1];
  integer calls;
  logic [7:0] got;
  function automatic int pick_index();
    calls = calls + 1;
    return 0;
  endfunction
  initial begin
    calls = 0;
    values[0] = tagged A(8'h11);
    values[1] = tagged A(8'h22);
    got = values[pick_index()].A;
    if (calls !== 1 || got !== 8'h11)
      $fatal(1, "selector calls=%0d got=%h", calls, got);
    $display("PASS r02_tagged_selector_once");
    $finish;
  end
endmodule
