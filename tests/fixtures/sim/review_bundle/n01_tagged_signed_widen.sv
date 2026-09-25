// Typed tagged members retain signedness through assignments and call boundaries.
module tb;
  typedef union tagged packed {
    logic signed [3:0] S;
    logic [3:0] U;
    bit signed [3:0] B;
  } value_t;
  typedef union tagged packed { value_t Data; logic [5:0] Other; } outer_t;
  value_t values[2];
  outer_t nested;
  logic signed [3:0] input_value;
  logic signed [7:0] signed_result;
  logic [7:0] unsigned_result;
  int calls;
  function automatic int pick();
    calls++;
    return 0;
  endfunction
  function automatic value_t make(input logic signed [3:0] value);
    return tagged S value;
  endfunction
  function automatic logic [7:0] via_formal(input value_t value);
    value_t local_value;
    local_value = value;
    return local_value.S;
  endfunction
  function automatic logic [7:0] take_byte(input logic [7:0] value);
    return value;
  endfunction
  initial begin
    calls = 0;
    input_value = -4'sd2;
    values[0] = make(input_value);
    values[1] = tagged U 4'he;
    signed_result = values[pick()].S;
    unsigned_result = values[0].S;
    if (calls !== 1 || signed_result !== -8'sd2 || unsigned_result !== 8'hfe)
      $fatal(1, "signed tagged read or selector capture");
    if (via_formal(make(input_value)) !== 8'hfe || take_byte(values[0].S) !== 8'hfe)
      $fatal(1, "tagged call/return conversion");
    signed_result = values[1].U;
    if (signed_result !== 8'sd14) $fatal(1, "unsigned member was sign-extended");
    nested = tagged Data (tagged S input_value);
    if (take_byte(nested.Data.S) !== 8'hfe) $fatal(1, "nested signed member");
    values[0] = tagged S 4'bx010;
    unsigned_result = values[0].S;
    if (unsigned_result !== 8'bxxxxx010) $fatal(1, "signed X extension");
    values[0] = tagged S 4'bz010;
    unsigned_result = values[0].S;
    if (unsigned_result !== 8'bzzzzz010) $fatal(1, "signed Z extension");
    values[0] = tagged B 4'b1z10;
    signed_result = values[0].B;
    if (signed_result !== -8'sd6) $fatal(1, "two-state signed member");
    $display("PASS n01_tagged_signed_widen");
    $finish(0);
  end
endmodule
