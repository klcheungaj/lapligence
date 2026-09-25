// SV 11.8.1: a concatenation is unsigned, even with one signed operand.
module tb;
  typedef logic [31:0] vector_t;
  logic signed [3:0] source_value;
  function automatic int widen(input logic signed [3:0] value);
    return int'({value});
  endfunction
  function automatic int direct(input logic signed [3:0] value);
    return int'(value);
  endfunction
  function automatic logic [64:0] wide(input logic signed [3:0] value);
    return {value};
  endfunction
  function automatic int unsigned_control(input logic [3:0] value);
    return int'({value});
  endfunction
  initial begin
    source_value = -4'sd2;
    if (widen(source_value) !== 14 || direct(source_value) !== -2)
      $fatal(1, "singleton concat signedness");
    if (wide(source_value) !== 65'd14 || unsigned_control(4'he) !== 14)
      $fatal(1, "wide/unsigned concat conversion");
    if (int'({'1}) !== 1 || vector_t'({'z}) !== {31'b0, 1'bz})
      $fatal(1, "concat must end an unbased fill context");
    source_value = 4'bx010;
    if (wide(source_value) !== {61'b0, 4'bx010})
      $fatal(1, "concat must not sign-extend unknowns");
    source_value = 4'bz010;
    if (wide(source_value) !== {61'b0, 4'bz010})
      $fatal(1, "concat must preserve low Z and zero-extend");
    $display("PASS n06_singleton_concat_signed_cast");
    $finish(0);
  end
endmodule
