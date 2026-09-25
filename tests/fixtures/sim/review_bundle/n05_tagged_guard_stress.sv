// N05: exercise both owned guards; sanitizer execution is required for leak evidence.
module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } inner_t;
  typedef union tagged packed { inner_t Data; logic [8:0] Other; } outer_t;
  outer_t values[0:1];
  logic [7:0] value;
  integer k;
  integer calls;
  function automatic int pick_index();
    calls = calls + 1;
    return 0;
  endfunction
  task automatic touch(ref outer_t data);
    data.Data.A = data.Data.A;
  endtask
  initial begin
    calls = 0;
    values[0] = tagged Data (tagged A 8'h5a);
    values[1] = tagged Data (tagged A 8'ha5);
    for (k = 0; k < 1000; k = k + 1) begin
      value = values[pick_index()].Data.A;
      values[pick_index()].Data.A = value;
      touch(values[pick_index()]);
      if (value !== 8'h5a || values[1].Data.A !== 8'ha5)
        $fatal(1, "tagged payload corrupted");
    end
    if (calls !== 3000) $fatal(1, "receiver evaluated more than once");
    $display("PASS n05_tagged_guard_stress");
    $finish(0);
  end
endmodule
